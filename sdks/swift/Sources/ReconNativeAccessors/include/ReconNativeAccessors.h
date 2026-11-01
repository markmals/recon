#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

/// The outcome of one allowlisted native read. The getter runs, and any Objective-C exception is
/// caught, entirely inside Objective-C before control returns to the Swift caller.
///
/// Every function here is the whole exception boundary for the getters it names. Swift sees a
/// status and a value; it never sees a selector, a key path, or an expression, and no caller can
/// supply one. Adding a field is a change to this allowlist.
typedef NS_ENUM(NSInteger, ReconReadStatus) {
    ReconReadStatusValue,
    ReconReadStatusUnavailable,
    ReconReadStatusFailed,
};

typedef NS_ENUM(NSInteger, ReconStringField) {
    ReconStringFieldTitle,
    ReconStringFieldToolTip,
    ReconStringFieldIdentifier,
    ReconStringFieldAccessibilityIdentifier,
    ReconStringFieldAccessibilityRole,
    ReconStringFieldLayerName,
    ReconStringFieldWindowAppearanceName,
    ReconStringFieldViewEffectiveAppearanceName,
    ReconStringFieldConstraintIdentifier,
};

typedef NS_ENUM(NSInteger, ReconBoolField) {
    ReconBoolFieldWindowVisible,
    ReconBoolFieldWindowKey,
    ReconBoolFieldWindowMain,
    ReconBoolFieldViewFlipped,
    ReconBoolFieldViewHidden,
    ReconBoolFieldLayerHidden,
    ReconBoolFieldLayerMasksToBounds,
    ReconBoolFieldCellEnabled,
    ReconBoolFieldCellEditable,
    ReconBoolFieldConstraintActive,
    ReconBoolFieldVisualEffectEmphasized,
};

typedef NS_ENUM(NSInteger, ReconIntegerField) {
    ReconIntegerFieldWindowNumber,
    ReconIntegerFieldCellType,
    ReconIntegerFieldCellState,
    ReconIntegerFieldViewSubviewCount,
    ReconIntegerFieldLayerSublayerCount,
    ReconIntegerFieldLayerMaskedCorners,
    ReconIntegerFieldVisualEffectMaterial,
    ReconIntegerFieldVisualEffectBlendingMode,
    ReconIntegerFieldVisualEffectState,
    ReconIntegerFieldConstraintFirstAttribute,
    ReconIntegerFieldConstraintSecondAttribute,
    ReconIntegerFieldConstraintRelation,
    ReconIntegerFieldAttributedStringLength,
    ReconIntegerFieldScreenNumber,
};

typedef NS_ENUM(NSInteger, ReconDoubleField) {
    ReconDoubleFieldViewAlpha,
    ReconDoubleFieldWindowBackingScaleFactor,
    ReconDoubleFieldLayerOpacity,
    ReconDoubleFieldLayerContentsScale,
    ReconDoubleFieldLayerCornerRadius,
    ReconDoubleFieldLayerBorderWidth,
    ReconDoubleFieldLayerShadowOpacity,
    ReconDoubleFieldLayerShadowRadius,
    ReconDoubleFieldViewHuggingHorizontal,
    ReconDoubleFieldViewHuggingVertical,
    ReconDoubleFieldViewCompressionHorizontal,
    ReconDoubleFieldViewCompressionVertical,
    ReconDoubleFieldConstraintMultiplier,
    ReconDoubleFieldConstraintConstant,
    ReconDoubleFieldConstraintPriority,
};

typedef NS_ENUM(NSInteger, ReconRectField) {
    ReconRectFieldWindowFrame,
    ReconRectFieldViewFrame,
    ReconRectFieldViewBounds,
    ReconRectFieldLayerFrame,
    ReconRectFieldLayerBounds,
    ReconRectFieldGuideFrame,
};

typedef NS_ENUM(NSInteger, ReconSizeField) {
    ReconSizeFieldViewIntrinsicContentSize,
};

typedef NS_ENUM(NSInteger, ReconObjectField) {
    ReconObjectFieldWindowContentView,
    ReconObjectFieldWindowAttachedSheet,
    ReconObjectFieldWindowParentWindow,
    ReconObjectFieldWindowScreen,
    ReconObjectFieldViewSuperview,
    ReconObjectFieldViewWindow,
    ReconObjectFieldViewLayer,
    ReconObjectFieldLayerSuperlayer,
    ReconObjectFieldLayerMask,
    ReconObjectFieldLayerDelegate,
    ReconObjectFieldGuideOwningView,
    ReconObjectFieldControlCell,
    ReconObjectFieldAttributedStringValue,
};

typedef NS_ENUM(NSInteger, ReconArrayField) {
    ReconArrayFieldViewSubviews,
    ReconArrayFieldViewConstraints,
    ReconArrayFieldViewLayoutGuides,
    ReconArrayFieldWindowChildWindows,
    ReconArrayFieldLayerSublayers,
};

typedef NS_ENUM(NSInteger, ReconColorField) {
    ReconColorFieldWindowBackgroundColor,
    ReconColorFieldTextFieldTextColor,
    ReconColorFieldTextFieldBackgroundColor,
    ReconColorFieldTextFieldCellTextColor,
    ReconColorFieldBoxFillColor,
    ReconColorFieldLayerBackgroundColor,
    ReconColorFieldLayerBorderColor,
    ReconColorFieldLayerShadowColor,
};

/// The live classes a facet distinguishes. A client never names one.
typedef NS_ENUM(NSInteger, ReconNativeClass) {
    ReconNativeClassWindow,
    ReconNativeClassView,
    ReconNativeClassLayer,
    ReconNativeClassCell,
    ReconNativeClassLayoutGuide,
    ReconNativeClassControl,
    ReconNativeClassTextView,
    ReconNativeClassTextField,
    ReconNativeClassTextFieldCell,
    ReconNativeClassBox,
    ReconNativeClassVisualEffectView,
};

/// The five object kinds Recon publishes handles for, and nothing else.
typedef NS_ENUM(NSInteger, ReconObjectKind) {
    ReconObjectKindUnsupported,
    ReconObjectKindWindow,
    ReconObjectKindView,
    ReconObjectKindLayer,
    ReconObjectKindCell,
    ReconObjectKindLayoutGuide,
};

/// One colour as the target resolved it, in the device RGB space where that conversion exists.
@interface ReconColorFacts : NSObject
@property (nonatomic, readonly, copy) NSString *colorSpace;
@property (nonatomic, readonly, copy, nullable) NSString *catalog;
@property (nonatomic, readonly, copy, nullable) NSString *name;
@property (nonatomic, readonly) BOOL converted;
@property (nonatomic, readonly) double red;
@property (nonatomic, readonly) double green;
@property (nonatomic, readonly) double blue;
@property (nonatomic, readonly) double alpha;
@end

/// One font as its own descriptor reports it.
@interface ReconFontFacts : NSObject
@property (nonatomic, readonly, copy) NSString *family;
@property (nonatomic, readonly, copy) NSString *postScriptName;
@property (nonatomic, readonly) double pointSize;
@property (nonatomic, readonly) NSUInteger symbolicTraits;
@property (nonatomic, readonly) BOOL hasWeight;
@property (nonatomic, readonly) double weight;
@property (nonatomic, readonly, copy) NSString *descriptorName;
@end

/// One longest-effective font run of an attributed string.
@interface ReconFontRun : NSObject
@property (nonatomic, readonly) NSUInteger location;
@property (nonatomic, readonly) NSUInteger length;
@property (nonatomic, readonly, nullable) ReconFontFacts *font;
@end

/// One layout constraint. `firstItem` and `secondItem` are live objects the caller turns into
/// handles; nothing else about them is read here.
@interface ReconConstraintFacts : NSObject
@property (nonatomic, readonly, weak, nullable) id firstItem;
@property (nonatomic, readonly, weak, nullable) id secondItem;
@property (nonatomic, readonly) NSInteger firstAttribute;
@property (nonatomic, readonly) NSInteger secondAttribute;
@property (nonatomic, readonly) NSInteger relation;
@property (nonatomic, readonly) double multiplier;
@property (nonatomic, readonly) double constant;
@property (nonatomic, readonly) double priority;
@property (nonatomic, readonly) BOOL active;
@property (nonatomic, readonly, copy, nullable) NSString *identifier;
@end

/// The 4x4 model transform of a layer, in column-major order as `CATransform3D` stores it.
@interface ReconTransformFacts : NSObject
@property (nonatomic, readonly, copy) NSArray<NSNumber *> *matrix;
@end

/// One WindowServer window the inventory reports, whoever owns it.
@interface ReconWindowServerEntry : NSObject
@property (nonatomic, readonly) uint32_t number;
@property (nonatomic, readonly) int32_t ownerPid;
@property (nonatomic, readonly) BOOL onscreen;
@end

/// The target process's own identity, as both the lifecycle response and an observation publish
/// it. One owner, because the two must never disagree.
@interface ReconProcessIdentityFacts : NSObject
@property (nonatomic, readonly) int32_t pid;
@property (nonatomic, readonly) uint32_t pidversion;
@property (nonatomic, readonly) uint32_t uid;
@property (nonatomic, readonly, copy, nullable) NSString *bundleId;
@property (nonatomic, readonly, copy, nullable) NSString *bundlePath;
@property (nonatomic, readonly, copy, nullable) NSString *executablePath;
@property (nonatomic, readonly, copy) NSString *architecture;
@property (nonatomic, readonly, copy) NSString *osBuild;
@property (nonatomic, readonly, copy) NSString *osVersion;
@end

ReconReadStatus ReconReadString(id object, ReconStringField field,
                                NSString *_Nullable *_Nonnull value);
ReconReadStatus ReconReadBool(id object, ReconBoolField field, BOOL *value);
ReconReadStatus ReconReadInteger(id object, ReconIntegerField field, NSInteger *value);
ReconReadStatus ReconReadDouble(id object, ReconDoubleField field, double *value);
ReconReadStatus ReconReadRect(id object, ReconRectField field, CGRect *value);
ReconReadStatus ReconReadSize(id object, ReconSizeField field, CGSize *value);
ReconReadStatus ReconReadObject(id object, ReconObjectField field, id _Nullable *_Nonnull value);
ReconReadStatus ReconReadArray(id object, ReconArrayField field,
                               NSArray *_Nullable *_Nonnull value);
ReconReadStatus ReconReadColor(id object, ReconColorField field,
                               ReconColorFacts *_Nullable *_Nonnull value);

/// Whether the object implements the getter at all, which an observation reports as an
/// unsupported field rather than an absent value.
BOOL ReconRespondsToString(id object, ReconStringField field);

/// The font a control, text view, or cell carries, if it carries one.
ReconReadStatus ReconReadCarrierFont(id object, ReconFontFacts *_Nullable *_Nonnull value);

/// The longest effective font run starting at `offset`. The caller advances and bounds the walk.
ReconReadStatus ReconReadFontRun(id attributedString, NSUInteger offset,
                                 ReconFontRun *_Nullable *_Nonnull value);

ReconReadStatus ReconReadConstraint(id constraint,
                                    ReconConstraintFacts *_Nullable *_Nonnull value);
ReconReadStatus ReconReadLayerTransform(id layer, ReconTransformFacts *_Nullable *_Nonnull value);

/// `view.bounds` in its window's coordinates, and that rectangle on the global screen.
ReconReadStatus ReconReadViewRectInWindow(id view, CGRect *value);
ReconReadStatus ReconConvertWindowRectToScreen(id window, CGRect rect, CGRect *value);

NSString *ReconClassName(id object);
NSArray<NSString *> *ReconSuperclassNames(id object, NSUInteger limit);
ReconObjectKind ReconKindOf(id object);
BOOL ReconIsKindOf(id object, ReconNativeClass nativeClass);

ReconReadStatus ReconReadApplicationWindows(NSArray *_Nullable *_Nonnull windows);
id _Nullable ReconApplicationWindowWithNumber(NSInteger number);
NSArray<ReconWindowServerEntry *> *ReconWindowServerInventory(void);

/// The height of the screen that owns the global origin, or NaN where no screen reports one.
double ReconPrimaryScreenHeight(void);
/// The rectangle flipped into accessibility's top-left origin. The flip is that screen's height
/// and nothing else: both spaces are logical points.
CGRect ReconScreenTopLeftRect(CGRect rect, double primaryScreenHeight);

ReconProcessIdentityFacts *ReconReadProcessIdentity(void);
/// The same facts as the lifecycle response's `identity` member.
NSDictionary *ReconProcessIdentityDictionary(void);

NS_ASSUME_NONNULL_END
