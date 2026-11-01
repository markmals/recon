#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

/// The outcome of one allowlisted native read. The getter runs, and any Objective-C exception is
/// caught, entirely inside Objective-C before control returns to the Swift caller.
typedef NS_ENUM(NSInteger, ReconReadStatus) {
    ReconReadStatusValue,
    ReconReadStatusUnavailable,
    ReconReadStatusFailed,
};

/// String-valued getters an extension may read. A client never supplies a selector.
typedef NS_ENUM(NSInteger, ReconStringField) {
    ReconStringFieldTitle,
    ReconStringFieldToolTip,
    ReconStringFieldIdentifier,
};

ReconReadStatus ReconReadString(id object, ReconStringField field, NSString *_Nullable *_Nonnull value);
ReconReadStatus ReconReadApplicationWindows(NSArray *_Nullable *_Nonnull windows);
ReconReadStatus ReconReadContentView(id window, id _Nullable *_Nonnull view);
ReconReadStatus ReconReadSubviews(id view, NSArray *_Nullable *_Nonnull subviews);
ReconReadStatus ReconReadWindowNumber(id window, NSInteger *number);
NSString *ReconClassName(id object);

NS_ASSUME_NONNULL_END
