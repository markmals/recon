#import "ReconInspect.h"
#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <objc/runtime.h>
#import <math.h>
static id scalar(CGFloat value) {
    return isfinite(value) ? @(value) : ReconStatus(@"non-finite");
}

NSDictionary *ReconFontDescriptor(NSFont *font, ReconWork *work) {
    if (!font) return ReconStatus(@"absent");
    NSFontDescriptor *descriptor = font.fontDescriptor;
    NSDictionary *traits = [descriptor objectForKey:NSFontTraitsAttribute];
    return @{ @"status": @"value", @"family": ReconBounded(font.familyName ?: @"", work),
              @"postScriptName": ReconBounded(font.fontName ?: @"", work),
              @"pointSize": scalar(font.pointSize), @"symbolicTraits": @((NSUInteger)descriptor.symbolicTraits),
              @"weight": traits[NSFontWeightTrait] ? scalar([traits[NSFontWeightTrait] doubleValue]) : ReconStatus(@"absent"),
              @"descriptor": ReconBounded(descriptor.postscriptName ?: @"", work) };
}
static NSFont *carrierFont(id object) {
    if ([object isKindOfClass:NSControl.class]) return ((NSControl *)object).font;
    if ([object isKindOfClass:NSTextView.class]) return ((NSTextView *)object).font;
    if ([object isKindOfClass:NSCell.class]) return ((NSCell *)object).font;
    return nil;
}
NSDictionary *ReconFont(id object, ReconWork *work) {
    NSFont *font = carrierFont(object);
    NSAttributedString *attributed = nil;
    if ([object isKindOfClass:NSControl.class]) attributed = ((NSControl *)object).attributedStringValue;
    if ([object isKindOfClass:NSCell.class]) attributed = ((NSCell *)object).attributedStringValue;
    NSMutableArray *runs = [NSMutableArray new];
    NSUInteger offset = 0, examined = 0;
    while (attributed && offset < attributed.length && examined < 256 && runs.count < work.limit &&
           [work consume:@"attributed-runs"]) {
        examined++;
        NSRange range;
        NSFont *runFont = [attributed attribute:NSFontAttributeName atIndex:offset longestEffectiveRange:&range inRange:NSMakeRange(0, attributed.length)];
        if (runFont) [runs addObject:@{ @"range": @{ @"location": @(range.location), @"length": @(range.length) },
                                       @"font": ReconFontDescriptor(runFont, work), @"source": @"attributed-run" }];
        offset = NSMaxRange(range);
    }
    if (attributed && offset < attributed.length && examined == 256) [work.truncated addObject:@"attributed-runs:256"];
    else if (attributed && offset < attributed.length && runs.count == work.limit) [work.truncated addObject:@"attributed-runs:limit"];
    if (!font && !runs.count) return ReconStatus(@"absent");
    return @{ @"status": @"value", @"carrier": font ? ReconFontDescriptor(font, work) : ReconStatus(@"absent"),
              @"source": [object isKindOfClass:NSCell.class] ? @"cell" : @"view", @"runs": runs };
}
static NSDictionary *metric(CGFloat value) {
    return value == NSViewNoIntrinsicMetric ? ReconStatus(@"no-intrinsic-metric") :
        (isfinite(value) ? @{ @"status": @"value", @"value": @(value) } : ReconStatus(@"non-finite"));
}
static id constraintItem(id item, ReconWork *work) {
    NSString *kind = ReconKind(item);
    if (!kind) return ReconStatus(@"unsupported");
    NSString *handle = [work reference:item];
    return handle ? @{ @"kind": kind, @"handle": handle } : ReconStatus(@"unavailable");
}
NSDictionary *ReconLayout(id object, ReconWork *work) {
    if (![object isKindOfClass:NSView.class] && ![object isKindOfClass:NSLayoutGuide.class]) return ReconStatus(@"unsupported");
    NSView *view = [object isKindOfClass:NSView.class] ? object : ((NSLayoutGuide *)object).owningView;
    if (!view) return ReconStatus(@"absent");
    NSMutableArray *constraints = [NSMutableArray new];
    NSHashTable *seen = [NSHashTable hashTableWithOptions:NSHashTableObjectPointerPersonality];
    NSHashTable *lineage = [NSHashTable hashTableWithOptions:NSHashTableObjectPointerPersonality];
    BOOL isGuide = [object isKindOfClass:NSLayoutGuide.class];
    for (NSView *owner = view; owner && ![work expired]; owner = owner.superview) {
        if (!isGuide) [lineage addObject:owner];
        if (constraints.count >= work.limit) break;
        for (NSLayoutConstraint *constraint in owner.constraints) {
            if (constraints.count >= work.limit) {
                [work.truncated addObject:@"layout-constraints:limit"];
                break;
            }
            if (![work consume:@"constraints"]) break;
            if ([seen containsObject:constraint]) continue;
            [seen addObject:constraint];
            BOOL touching = (!isGuide && owner == view) || constraint.firstItem == object || constraint.secondItem == object ||
                (!isGuide && ([lineage containsObject:constraint.firstItem] || [lineage containsObject:constraint.secondItem]));
            if (!touching) continue;
            [constraints addObject:@{ @"owner": constraintItem(owner, work),
                @"firstItem": constraintItem(constraint.firstItem, work),
                @"secondItem": constraint.secondItem ? constraintItem(constraint.secondItem, work) : ReconStatus(@"absent"),
                @"firstAttribute": @(constraint.firstAttribute), @"secondAttribute": @(constraint.secondAttribute),
                @"relation": @(constraint.relation), @"multiplier": scalar(constraint.multiplier),
                @"constant": scalar(constraint.constant), @"priority": scalar(constraint.priority),
                @"active": @(constraint.active),
                @"identifier": constraint.identifier ? ReconBounded(constraint.identifier, work) : ReconStatus(@"absent") }];
        }
        if (work.visited >= 25000 || constraints.count >= work.limit) break;
    }
    NSMutableArray *guides = [NSMutableArray new];
    if (!isGuide) for (NSLayoutGuide *guide in view.layoutGuides) {
        if (guides.count >= work.limit) {
            [work.truncated addObject:@"layout-guides:limit"];
            break;
        }
        if (![work consume:@"layout-guides"]) break;
        [guides addObject:@{ @"kind": @"layoutGuide", @"handle": [work reference:guide] ?: ReconStatus(@"unavailable") }];
    }
    if (isGuide) return @{ @"status": @"value", @"constraints": constraints,
        @"layoutGuides": @[], @"intrinsicWidth": ReconStatus(@"unsupported"),
        @"intrinsicHeight": ReconStatus(@"unsupported"), @"hugging": ReconStatus(@"unsupported"),
        @"compressionResistance": ReconStatus(@"unsupported") };
    NSSize size = view.intrinsicContentSize;
    return @{ @"status": @"value", @"constraints": constraints, @"layoutGuides": guides,
        @"intrinsicWidth": metric(size.width), @"intrinsicHeight": metric(size.height),
        @"hugging": @{ @"horizontal": scalar([view contentHuggingPriorityForOrientation:NSLayoutConstraintOrientationHorizontal]),
                       @"vertical": scalar([view contentHuggingPriorityForOrientation:NSLayoutConstraintOrientationVertical]) },
        @"compressionResistance": @{ @"horizontal": scalar([view contentCompressionResistancePriorityForOrientation:NSLayoutConstraintOrientationHorizontal]),
                                     @"vertical": scalar([view contentCompressionResistancePriorityForOrientation:NSLayoutConstraintOrientationVertical]) } };
}
static NSDictionary *transform(CATransform3D t) {
    const CGFloat values[] = {t.m11,t.m12,t.m13,t.m14,t.m21,t.m22,t.m23,t.m24,
                              t.m31,t.m32,t.m33,t.m34,t.m41,t.m42,t.m43,t.m44};
    NSMutableArray *numbers = [NSMutableArray new];
    for (NSUInteger i = 0; i < 16; i++) {
        if (!isfinite(values[i])) return ReconStatus(@"non-finite");
        [numbers addObject:@(values[i])];
    }
    return @{ @"status": @"value", @"matrix": numbers };
}
NSDictionary *ReconLayer(id object, ReconWork *work) {
    CALayer *layer = nil;
    if ([object isKindOfClass:NSView.class]) layer = ((NSView *)object).layer;
    else if ([object isKindOfClass:CALayer.class]) layer = object;
    else if ([object isKindOfClass:NSWindow.class]) layer = ((NSWindow *)object).contentView.layer;
    else return ReconStatus(@"unsupported");
    if (!layer) return ReconStatus(@"absent");
    NSMutableDictionary *result = [@{ @"status": @"value", @"layerState": @"model",
        @"handle": [work reference:layer] ?: ReconStatus(@"unavailable"),
        @"frame": ReconRect(NSRectFromCGRect(layer.frame), @"parent-model-layer", @"layer-geometry", work),
        @"bounds": ReconRect(NSRectFromCGRect(layer.bounds), @"self-model-layer", @"layer-geometry", work),
        @"transform": transform(layer.transform), @"contentsScale": scalar(layer.contentsScale),
        @"hidden": @(layer.hidden), @"opacity": scalar(layer.opacity),
        @"cornerRadius": scalar(layer.cornerRadius), @"maskedCorners": @(layer.maskedCorners),
        @"borderWidth": scalar(layer.borderWidth), @"shadowOpacity": scalar(layer.shadowOpacity),
        @"shadowRadius": scalar(layer.shadowRadius), @"masksToBounds": @(layer.masksToBounds),
        @"mask": [work reference:layer.mask] ?: ReconStatus(@"absent") } mutableCopy];
    if (layer.backgroundColor) result[@"backgroundColor"] = ReconColor([NSColor colorWithCGColor:layer.backgroundColor], work);
    else result[@"backgroundColor"] = ReconStatus(@"absent");
    if (layer.borderColor) result[@"borderColor"] = ReconColor([NSColor colorWithCGColor:layer.borderColor], work);
    else result[@"borderColor"] = ReconStatus(@"absent");
    if (layer.shadowColor) result[@"shadowColor"] = ReconColor([NSColor colorWithCGColor:layer.shadowColor], work);
    else result[@"shadowColor"] = ReconStatus(@"absent");
    return result;
}
NSDictionary *ReconCell(id object, ReconWork *work) {
    NSCell *cell = nil;
    if ([object isKindOfClass:NSControl.class]) cell = ((NSControl *)object).cell;
    else if ([object isKindOfClass:NSCell.class]) cell = object;
    else return ReconStatus(@"unsupported");
    if (!cell) return ReconStatus(@"absent");
    return @{ @"status": @"value", @"handle": [work reference:cell] ?: ReconStatus(@"unavailable"),
        @"class": ReconBounded(NSStringFromClass(object_getClass(cell)), work),
        @"cellType": @(cell.type), @"state": @(cell.state),
        @"enabled": @(cell.isEnabled), @"editable": @(cell.isEditable),
        @"font": ReconFontDescriptor(cell.font, work) };
}
