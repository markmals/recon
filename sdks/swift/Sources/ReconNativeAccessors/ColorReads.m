#import "ReconAccessorsInternal.h"

/// `colorSpace`, `catalogNameComponent` and `colorNameComponent` each raise on the colours the
/// other kind owns — a catalog colour has no colour space, a device colour has no catalog — so
/// reading any colour raises one of them, and the facet that asked reports a failed target
/// getter. That is what the Objective-C walker this replaces does, and what its payloads say.

static NSColor *_Nullable layerColor(CALayer *layer, ReconColorField field) {
    CGColorRef color = NULL;
    switch (field) {
        case ReconColorFieldLayerBackgroundColor: color = layer.backgroundColor; break;
        case ReconColorFieldLayerBorderColor: color = layer.borderColor; break;
        case ReconColorFieldLayerShadowColor: color = layer.shadowColor; break;
        default: return nil;
    }
    return color ? [NSColor colorWithCGColor:color] : nil;
}

static NSColor *_Nullable colorForField(id object, ReconColorField field) {
    switch (field) {
        case ReconColorFieldWindowBackgroundColor:
            return ReconIsKindOf(object, ReconNativeClassWindow) ? [(NSWindow *)object backgroundColor] : nil;
        case ReconColorFieldTextFieldTextColor:
            return ReconIsKindOf(object, ReconNativeClassTextField) ? [(NSTextField *)object textColor] : nil;
        case ReconColorFieldTextFieldBackgroundColor:
            return ReconIsKindOf(object, ReconNativeClassTextField) ? [(NSTextField *)object backgroundColor] : nil;
        case ReconColorFieldTextFieldCellTextColor:
            return ReconIsKindOf(object, ReconNativeClassTextFieldCell) ? [(NSTextFieldCell *)object textColor] : nil;
        case ReconColorFieldBoxFillColor:
            return ReconIsKindOf(object, ReconNativeClassBox) ? [(NSBox *)object fillColor] : nil;
        case ReconColorFieldLayerBackgroundColor:
        case ReconColorFieldLayerBorderColor:
        case ReconColorFieldLayerShadowColor:
            return ReconIsKindOf(object, ReconNativeClassLayer) ? layerColor(object, field) : nil;
    }
}

static ReconColorFacts *colorFacts(NSColor *color) {
    ReconColorFacts *facts = [ReconColorFacts new];
    facts.colorSpace = color.colorSpace.localizedName ?: @"unknown";
    facts.catalog = color.catalogNameComponent;
    facts.name = color.colorNameComponent;
    NSColor *converted = [color colorUsingColorSpace:NSColorSpace.deviceRGBColorSpace];
    facts.converted = converted != nil;
    if (converted) {
        facts.red = converted.redComponent;
        facts.green = converted.greenComponent;
        facts.blue = converted.blueComponent;
        facts.alpha = converted.alphaComponent;
    }
    return facts;
}

ReconReadStatus ReconReadColor(id object, ReconColorField field, ReconColorFacts **value) {
    *value = nil;
    @try {
        NSColor *color = colorForField(object, field);
        if (!color) return ReconReadStatusUnavailable;
        *value = colorFacts(color);
        return ReconReadStatusValue;
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}
