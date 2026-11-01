#import "ReconNativeAccessors.h"

#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>

#if !__has_feature(objc_arc)
#error "ReconNativeAccessors requires ARC"
#endif

NS_ASSUME_NONNULL_BEGIN

/// The fact objects are immutable to their caller and assembled by the reads in this directory,
/// so every property is writable here and nowhere else.

@interface ReconColorFacts ()
@property (nonatomic, readwrite, copy) NSString *colorSpace;
@property (nonatomic, readwrite, copy, nullable) NSString *catalog;
@property (nonatomic, readwrite, copy, nullable) NSString *name;
@property (nonatomic, readwrite) BOOL converted;
@property (nonatomic, readwrite) double red;
@property (nonatomic, readwrite) double green;
@property (nonatomic, readwrite) double blue;
@property (nonatomic, readwrite) double alpha;
@end

@interface ReconFontFacts ()
@property (nonatomic, readwrite, copy) NSString *family;
@property (nonatomic, readwrite, copy) NSString *postScriptName;
@property (nonatomic, readwrite) double pointSize;
@property (nonatomic, readwrite) NSUInteger symbolicTraits;
@property (nonatomic, readwrite) BOOL hasWeight;
@property (nonatomic, readwrite) double weight;
@property (nonatomic, readwrite, copy) NSString *descriptorName;
@end

@interface ReconFontRun ()
@property (nonatomic, readwrite) NSUInteger location;
@property (nonatomic, readwrite) NSUInteger length;
@property (nonatomic, readwrite, nullable) ReconFontFacts *font;
@end

@interface ReconConstraintFacts ()
@property (nonatomic, readwrite, weak, nullable) id firstItem;
@property (nonatomic, readwrite, weak, nullable) id secondItem;
@property (nonatomic, readwrite) NSInteger firstAttribute;
@property (nonatomic, readwrite) NSInteger secondAttribute;
@property (nonatomic, readwrite) NSInteger relation;
@property (nonatomic, readwrite) double multiplier;
@property (nonatomic, readwrite) double constant;
@property (nonatomic, readwrite) double priority;
@property (nonatomic, readwrite) BOOL active;
@property (nonatomic, readwrite, copy, nullable) NSString *identifier;
@end

@interface ReconTransformFacts ()
@property (nonatomic, readwrite, copy) NSArray<NSNumber *> *matrix;
@end

@interface ReconWindowServerEntry ()
@property (nonatomic, readwrite) uint32_t number;
@property (nonatomic, readwrite) int32_t ownerPid;
@property (nonatomic, readwrite) BOOL onscreen;
@end

@interface ReconProcessIdentityFacts ()
@property (nonatomic, readwrite) int32_t pid;
@property (nonatomic, readwrite) uint32_t pidversion;
@property (nonatomic, readwrite) uint32_t uid;
@property (nonatomic, readwrite, copy, nullable) NSString *bundleId;
@property (nonatomic, readwrite, copy, nullable) NSString *bundlePath;
@property (nonatomic, readwrite, copy, nullable) NSString *executablePath;
@property (nonatomic, readwrite, copy) NSString *architecture;
@property (nonatomic, readwrite, copy) NSString *osBuild;
@property (nonatomic, readwrite, copy) NSString *osVersion;
@end

NS_ASSUME_NONNULL_END
