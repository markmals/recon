#import "ReconAccessorsInternal.h"

static ReconFontFacts *fontFacts(NSFont *font) {
    NSFontDescriptor *descriptor = font.fontDescriptor;
    NSNumber *weight = [descriptor objectForKey:NSFontTraitsAttribute][NSFontWeightTrait];
    ReconFontFacts *facts = [ReconFontFacts new];
    facts.family = font.familyName ?: @"";
    facts.postScriptName = font.fontName ?: @"";
    facts.pointSize = font.pointSize;
    facts.symbolicTraits = (NSUInteger)descriptor.symbolicTraits;
    facts.hasWeight = weight != nil;
    facts.weight = weight.doubleValue;
    facts.descriptorName = descriptor.postscriptName ?: @"";
    return facts;
}

static NSFont *_Nullable carrierFont(id object) {
    if (ReconIsKindOf(object, ReconNativeClassControl)) return [(NSControl *)object font];
    if (ReconIsKindOf(object, ReconNativeClassTextView)) return [(NSTextView *)object font];
    if (ReconIsKindOf(object, ReconNativeClassCell)) return [(NSCell *)object font];
    return nil;
}

ReconReadStatus ReconReadCarrierFont(id object, ReconFontFacts **value) {
    *value = nil;
    @try {
        NSFont *font = carrierFont(object);
        if (!font) return ReconReadStatusUnavailable;
        *value = fontFacts(font);
        return ReconReadStatusValue;
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}

/// A run with no font attribute is still a run: its extent is what advances the caller's walk.
static ReconFontRun *runFacts(NSRange range, NSFont *_Nullable font) {
    ReconFontRun *run = [ReconFontRun new];
    run.location = range.location;
    run.length = range.length;
    run.font = font ? fontFacts(font) : nil;
    return run;
}

ReconReadStatus ReconReadFontRun(id attributedString, NSUInteger offset, ReconFontRun **value) {
    *value = nil;
    @try {
        if (![attributedString isKindOfClass:NSAttributedString.class]) return ReconReadStatusUnavailable;
        NSAttributedString *attributed = attributedString;
        if (offset >= attributed.length) return ReconReadStatusUnavailable;
        NSRange range;
        NSFont *font = [attributed attribute:NSFontAttributeName
                                     atIndex:offset
                       longestEffectiveRange:&range
                                     inRange:NSMakeRange(0, attributed.length)];
        *value = runFacts(range, font);
        return ReconReadStatusValue;
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}
