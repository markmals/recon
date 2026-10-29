#import "ReconInspect.h"
#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <objc/runtime.h>
#import <math.h>

NSDictionary *ReconStatus(NSString *status) { return @{ @"status": status }; }
NSString *ReconBounded(id value, ReconWork *work) {
    if (![value isKindOfClass:NSString.class]) return nil;
    NSString *bounded = ReconJSONString(value, 4096);
    if (bounded.length < [value length]) [work.truncated addObject:@"string:4096-scalars"];
    return bounded;
}
static NSDictionary *finiteNumber(CGFloat value) {
    return isfinite(value) ? @{ @"status": @"value", @"value": @(value) } : ReconStatus(@"non-finite");
}
NSDictionary *ReconRect(NSRect rect, NSString *space, NSString *orientation, ReconWork *work) {
    if (!isfinite(rect.origin.x) || !isfinite(rect.origin.y) ||
        !isfinite(rect.size.width) || !isfinite(rect.size.height)) return ReconStatus(@"non-finite");
    return @{ @"status": @"value", @"space": space, @"units": @"logical-points",
              @"orientation": orientation ?: @"unknown",
              @"x": @(rect.origin.x), @"y": @(rect.origin.y),
              @"width": @(rect.size.width), @"height": @(rect.size.height) };
}
static NSArray *superclasses(id object, ReconWork *work) {
    NSMutableArray *classes = [NSMutableArray new];
    Class cls = class_getSuperclass(object_getClass(object));
    while (cls && classes.count < 32) {
        [classes addObject:ReconBounded(NSStringFromClass(cls), work)];
        cls = class_getSuperclass(cls);
    }
    return classes;
}
NSDictionary *ReconBasic(id object, ReconWork *work) {
    NSMutableDictionary *result = [@{ @"status": @"value",
        @"class": ReconBounded(NSStringFromClass(object_getClass(object)), work),
        @"superclasses": superclasses(object, work) } mutableCopy];
    if ([object isKindOfClass:NSWindow.class]) {
        NSWindow *window = object;
        result[@"title"] = window.title ? @{ @"status": @"value", @"value": ReconBounded(window.title, work) } : ReconStatus(@"absent");
        result[@"identifier"] = window.identifier ? @{ @"status": @"value", @"value": ReconBounded(window.identifier, work) } : ReconStatus(@"absent");
        result[@"frame"] = ReconRect(window.frame, @"global-screen", @"bottom-left", work);
        result[@"visible"] = @(window.isVisible);
        result[@"key"] = @(window.isKeyWindow);
        result[@"main"] = @(window.isMainWindow);
        result[@"windowNumber"] = window.windowNumber > 0 ? @{ @"status": @"value", @"value": @(window.windowNumber) } : ReconStatus(@"absent");
        result[@"contentView"] = [work reference:window.contentView] ?: ReconStatus(@"absent");
        NSMutableArray *children = [NSMutableArray new];
        for (NSWindow *child in window.childWindows ?: @[]) {
            if (children.count >= work.limit) {
                [work.truncated addObject:@"child-windows:limit"];
                break;
            }
            if (![work consume:@"window-relationships"]) break;
            NSString *ref = [work reference:child]; if (ref) [children addObject:ref];
        }
        result[@"childWindows"] = children;
        result[@"sheet"] = [work reference:window.attachedSheet] ?: ReconStatus(@"absent");
        NSView *top = window.contentView;
        while (top.superview && top.superview.window == window && [work consume:@"window-frame-chain"]) top = top.superview;
        result[@"topmostView"] = [work reference:top] ?: ReconStatus(@"absent");
    } else if ([object isKindOfClass:NSView.class]) {
        NSView *view = object;
        NSView *parent = view.superview;
        result[@"identifier"] = view.identifier ? @{ @"status": @"value", @"value": ReconBounded(view.identifier, work) } : ReconStatus(@"absent");
        result[@"frame"] = ReconRect(view.frame, @"superview", parent ? (parent.isFlipped ? @"top-left" : @"bottom-left") : @"unattached", work);
        result[@"bounds"] = ReconRect(view.bounds, @"self", view.isFlipped ? @"top-left" : @"bottom-left", work);
        result[@"flipped"] = @(view.isFlipped);
        result[@"hidden"] = @(view.isHidden);
        result[@"alpha"] = finiteNumber(view.alphaValue);
        result[@"childCount"] = @(view.subviews.count);
        result[@"superview"] = [work reference:parent] ?: ReconStatus(@"absent");
        NSWindow *associated = view.window;
        result[@"window"] = [work reference:associated] ?: ReconStatus(@"absent");
        if (associated) {
            NSRect windowRect = [view convertRect:view.bounds toView:nil];
            NSRect screenRect = [associated convertRectToScreen:windowRect];
            result[@"normalizedFrame"] = @{ @"window": ReconRect(windowRect, @"window", @"bottom-left", work),
                @"screen": ReconRect(screenRect, @"global-screen", @"bottom-left", work),
                @"backingScaleFactor": finiteNumber(associated.backingScaleFactor),
                @"displayId": associated.screen.deviceDescription[@"NSScreenNumber"] ?: ReconStatus(@"absent") };
        } else result[@"normalizedFrame"] = ReconStatus(@"absent");
        NSString *name = NSStringFromClass(object_getClass(view));
        if ([name containsString:@"HostingView"] || [name containsString:@"SwiftUI"]) {
            result[@"hostingBoundary"] = @{ @"evidence": @"runtime-class-name-heuristic", @"swiftUIContent": @"not-inspected" };
        }
    } else if ([object isKindOfClass:CALayer.class]) {
        CALayer *layer = object;
        result[@"frame"] = ReconRect(NSRectFromCGRect(layer.frame), @"parent-model-layer", @"layer-geometry", work);
        result[@"bounds"] = ReconRect(NSRectFromCGRect(layer.bounds), @"self-model-layer", @"layer-geometry", work);
        result[@"hidden"] = @(layer.hidden);
        result[@"opacity"] = finiteNumber(layer.opacity);
        result[@"childCount"] = @(layer.sublayers.count);
        result[@"superlayer"] = [work reference:layer.superlayer] ?: ReconStatus(@"absent");
        result[@"layerState"] = @"model";
    } else if ([object isKindOfClass:NSLayoutGuide.class]) {
        NSLayoutGuide *guide = object;
        result[@"identifier"] = guide.identifier ? @{ @"status": @"value", @"value": ReconBounded(guide.identifier, work) } : ReconStatus(@"absent");
        result[@"frame"] = ReconRect(guide.frame, @"owning-view", guide.owningView.isFlipped ? @"top-left" : @"bottom-left", work);
        result[@"owningView"] = [work reference:guide.owningView] ?: ReconStatus(@"absent");
    } else if ([object isKindOfClass:NSCell.class]) {
        NSCell *cell = object;
        result[@"enabled"] = @(cell.isEnabled);
        result[@"editable"] = @(cell.isEditable);
        result[@"cellType"] = @(cell.type);
        result[@"state"] = @(cell.state);
    }
    return result;
}

NSDictionary *ReconColor(NSColor *color, ReconWork *work) {
    if (!color) return ReconStatus(@"absent");
    NSMutableDictionary *result = [@{ @"status": @"value", @"colorSpace": ReconBounded(color.colorSpace.localizedName ?: @"unknown", work) } mutableCopy];
    if (color.catalogNameComponent) result[@"catalog"] = ReconBounded(color.catalogNameComponent, work);
    if (color.colorNameComponent) result[@"name"] = ReconBounded(color.colorNameComponent, work);
    NSColor *converted = [color colorUsingColorSpace:NSColorSpace.deviceRGBColorSpace];
    if (converted) {
        result[@"resolvedSpace"] = @"deviceRGB";
        result[@"components"] = @{ @"red": finiteNumber(converted.redComponent), @"green": finiteNumber(converted.greenComponent),
                                    @"blue": finiteNumber(converted.blueComponent), @"alpha": finiteNumber(converted.alphaComponent) };
    } else result[@"conversion"] = ReconStatus(@"unsupported");
    return result;
}
NSDictionary *ReconAppearance(id object, ReconWork *work) {
    if (![object isKindOfClass:NSView.class] && ![object isKindOfClass:NSWindow.class] && ![object isKindOfClass:NSCell.class]) return ReconStatus(@"unsupported");
    NSMutableDictionary *facts = [@{ @"status": @"value" } mutableCopy];
    if ([object isKindOfClass:NSView.class]) {
        NSView *view = object;
        facts[@"effectiveAppearance"] = view.effectiveAppearance.name ? ReconBounded(view.effectiveAppearance.name, work) : ReconStatus(@"absent");
        if ([view isKindOfClass:NSVisualEffectView.class]) {
            NSVisualEffectView *effect = (id)view;
            facts[@"visualEffect"] = @{ @"material": @(effect.material), @"blendingMode": @(effect.blendingMode),
                                         @"state": @(effect.state), @"emphasized": @(effect.emphasized) };
        }
        if ([view isKindOfClass:NSTextField.class]) {
            NSTextField *field = (id)view;
            facts[@"textColor"] = ReconColor(field.textColor, work);
            facts[@"backgroundColor"] = ReconColor(field.backgroundColor, work);
        } else if ([view isKindOfClass:NSBox.class]) facts[@"fillColor"] = ReconColor(((NSBox *)view).fillColor, work);
    } else if ([object isKindOfClass:NSWindow.class]) {
        NSWindow *window = object;
        facts[@"appearance"] = window.appearance.name ? ReconBounded(window.appearance.name, work) : ReconStatus(@"absent");
        facts[@"backgroundColor"] = ReconColor(window.backgroundColor, work);
    } else if ([object isKindOfClass:NSTextFieldCell.class]) {
        facts[@"textColor"] = ReconColor(((NSTextFieldCell *)object).textColor, work);
    }
    return facts;
}
