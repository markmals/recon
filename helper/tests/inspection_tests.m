#import "../ReconInspect.h"
#import <QuartzCore/QuartzCore.h>
#import <mach/mach_time.h>
#include <assert.h>

/// Defined by ReconFields.m. The flip is the one piece of the top-left normalization that can be
/// exercised without a window server session, so it is tested on its own numbers.
extern NSRect ReconScreenTopLeftRect(NSRect rect, CGFloat primaryScreenHeight);

@interface RepeatedView : NSView @end
@implementation RepeatedView
- (NSArray<NSView *> *)subviews {
    NSArray<NSView *> *children = [super subviews];
    return children.count ? @[ children.firstObject, children.firstObject ] : children;
}
@end

@interface AlternatingFontCell : NSCell @end
@implementation AlternatingFontCell
- (NSAttributedString *)attributedStringValue {
    NSMutableAttributedString *value = [[NSMutableAttributedString alloc] initWithString:
        [@"" stringByPaddingToLength:300 withString:@"a" startingAtIndex:0]];
    NSFont *font = [NSFont systemFontOfSize:12];
    for (NSUInteger i = 1; i < 300; i += 2)
        [value addAttribute:NSFontAttributeName value:font range:NSMakeRange(i, 1)];
    return value;
}
@end

static NSDictionary *inspect(ReconRegistry *registry, NSDictionary *request) {
    ReconWork *work = [ReconWork new];
    work.registry = registry; work.request = request; work.sessionId = @"fixture-session";
    work.depth = request[@"depth"] ? [request[@"depth"] unsignedIntegerValue] : 3;
    work.limit = request[@"limit"] ? [request[@"limit"] unsignedIntegerValue] : 200;
    mach_timebase_info_data_t scale; mach_timebase_info(&scale);
    work.deadline = mach_continuous_time() * scale.numer / scale.denom + 2000000000ULL;
    return ReconInspect(work);
}
int main(void) {
    @autoreleasepool {
        NSView *parent = [NSView new], *child = [NSView new];
        parent.identifier = @"parent"; child.identifier = @"child";
        [parent addSubview:child];
        ReconRegistry *registry = [ReconRegistry new];
        NSString *root = [registry handleForObject:parent];
        NSLayoutGuide *guide = [NSLayoutGuide new];
        guide.identifier = @"fixture-guide";
        [parent addLayoutGuide:guide];
        NSDictionary *layout = inspect(registry, @{ @"op": @"node", @"root": root,
            @"fields": @[ @"layout" ] });
        assert([layout[@"node"][@"layout"][@"layoutGuides"] count] == 1);
        NSString *guideHandle = layout[@"node"][@"layout"][@"layoutGuides"][0][@"handle"];
        NSDictionary *guideNode = inspect(registry, @{ @"op": @"node", @"root": guideHandle,
            @"fields": @[ @"basic" ] });
        assert([guideNode[@"node"][@"kind"] isEqual:@"layoutGuide"]);
        assert([guideNode[@"node"][@"relationships"][@"owningView"] isEqual:root]);
        ReconWork *runs = [ReconWork new];
        runs.registry = registry; runs.limit = 200;
        mach_timebase_info_data_t scale; mach_timebase_info(&scale);
        runs.deadline = mach_continuous_time() * scale.numer / scale.denom + 2000000000ULL;
        ReconFont([AlternatingFontCell new], runs);
        assert(runs.visited <= 256);
        assert([runs.truncated containsObject:@"attributed-runs:256"]);
        NSDictionary *zero = inspect(registry, @{ @"op": @"tree", @"root": root, @"depth": @0,
            @"fields": @[ @"basic" ] });
        assert([zero[@"ok"] boolValue]);
        assert([zero[@"nodes"] count] == 1);
        assert([zero[@"coverage"][@"narrowed"] containsObject:@"depth"]);
        NSDictionary *find = inspect(registry, @{ @"op": @"find", @"root": root,
            @"identifier": @"child", @"depth": @2, @"fields": @[ @"basic" ] });
        assert([find[@"nodes"] count] == 1);
        NSString *handle = find[@"nodes"][0][@"handle"];
        assert([handle isEqual:[registry handleForObject:child]]);
        NSDictionary *node = inspect(registry, @{ @"op": @"node", @"root": handle,
            @"fields": @[ @"appearance" ] });
        assert(node[@"node"][@"appearance"]);
        assert([node[@"node"][@"relationships"][@"superview"] isEqual:root]);
        assert([node[@"node"][@"relationships"][@"window"][@"status"] isEqual:@"absent"]);
        assert(!node[@"node"][@"basic"]);
        NSDictionary *absent = inspect(registry, @{ @"op": @"tree", @"root": root,
            @"kind": @"layer", @"fields": @[ @"basic" ] });
        assert([absent[@"ok"] boolValue]);
        assert([absent[@"nodes"] count] == 0 || [absent[@"nodes"][0][@"kind"] isEqual:@"layer"]);
        RepeatedView *duplicate = [RepeatedView new];
        NSView *member = [NSView new];
        member.identifier = @"member";
        [duplicate addSubview:member];
        NSString *duplicateRoot = [registry handleForObject:duplicate];
        NSDictionary *limited = inspect(registry, @{ @"op": @"tree", @"root": duplicateRoot,
            @"depth": @1, @"limit": @2, @"fields": @[ @"appearance" ] });
        assert([limited[@"nodes"] count] == 2);
        NSDictionary *references = inspect(registry, @{ @"op": @"tree", @"root": duplicateRoot,
            @"depth": @1, @"limit": @3, @"fields": @[ @"appearance" ] });
        assert([references[@"nodes"] count] == 3);
        assert([references[@"coverage"][@"returned"] unsignedIntegerValue] == 3);
        assert([limited[@"coverage"][@"truncated"] containsObject:@"unsearched-scope"]);
        NSDictionary *notFound = inspect(registry, @{ @"op": @"find", @"root": duplicateRoot,
            @"depth": @1, @"identifier": @"nomatch", @"fields": @[ @"basic" ] });
        assert([notFound[@"nodes"] count] == 0);

        // Accessibility's own properties are read in the target and are not NSView.identifier.
        NSView *labeled = [NSView new];
        labeled.identifier = @"native-identifier";
        labeled.accessibilityIdentifier = @"ax-identifier";
        labeled.accessibilityRole = NSAccessibilityButtonRole;
        NSDictionary *labels = inspect(registry, @{ @"op": @"node",
            @"root": [registry handleForObject:labeled], @"fields": @[ @"basic" ] })[@"node"][@"basic"];
        assert([labels[@"identifier"][@"value"] isEqual:@"native-identifier"]);
        assert([labels[@"accessibilityIdentifier"][@"value"] isEqual:@"ax-identifier"]);
        assert([labels[@"accessibilityIdentifier"][@"read"] isEqual:@"target-executed"]);
        assert([labels[@"accessibilityRole"][@"value"] isEqual:NSAccessibilityButtonRole]);
        // A view in no window has no normalization to report, which is an absence, not a rectangle.
        assert([labels[@"normalizedFrame"][@"status"] isEqual:@"absent"]);

        // AppKit's empty identifier is the absence of one: reporting it as a value would relate
        // every unidentified view to every unidentified accessibility element.
        NSDictionary *unlabeled = inspect(registry, @{ @"op": @"node",
            @"root": [registry handleForObject:[NSView new]], @"fields": @[ @"basic" ] })[@"node"][@"basic"];
        assert([unlabeled[@"accessibilityIdentifier"][@"status"] isEqual:@"absent"]);
        assert(!unlabeled[@"accessibilityIdentifier"][@"value"]);

        // The primary screen's height is the whole transform: a rectangle 100 points above the
        // bottom of a 900-point screen sits 780 points below its top.
        NSRect flipped = ReconScreenTopLeftRect(NSMakeRect(10, 100, 50, 20), 900);
        assert(NSEqualRects(flipped, NSMakeRect(10, 780, 50, 20)));
        // A display above and left of the primary one produces negative coordinates, which are a
        // position rather than an error.
        NSRect second = ReconScreenTopLeftRect(NSMakeRect(-1520, 1056, 80, 24), 900);
        assert(NSEqualRects(second, NSMakeRect(-1520, -180, 80, 24)));
        // Logical points only: the flip is its own inverse whatever the display's backing scale.
        NSRect retina = NSMakeRect(12.5, 33.25, 100.5, 50.75);
        assert(NSEqualRects(ReconScreenTopLeftRect(ReconScreenTopLeftRect(retina, 1050.5), 1050.5), retina));
        [registry revoke];
        assert([inspect(registry, @{ @"op": @"node", @"root": handle })[@"error"][@"code"] isEqual:@"STALE_NODE"]);
    }
    return 0;
}
