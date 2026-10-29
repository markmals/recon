#import "../ReconInspect.h"
#import <QuartzCore/QuartzCore.h>
#import <mach/mach_time.h>
#include <assert.h>

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
        [registry revoke];
        assert([inspect(registry, @{ @"op": @"node", @"root": handle })[@"error"][@"code"] isEqual:@"STALE_NODE"]);
    }
    return 0;
}
