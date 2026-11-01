#import <CoreGraphics/CoreGraphics.h>
#import <Foundation/Foundation.h>
#import <QuartzCore/CATransform3D.h>

NS_ASSUME_NONNULL_BEGIN

/// The whole Objective-C exception boundary between Recon and an inspected application.
///
/// Each function sends exactly one message to one object and answers what happened. An
/// application may implement any of those methods itself, so the send — and the check that the
/// object implements it at all — happen inside `@try`, and no Objective-C exception can reach
/// the Swift caller.
///
/// There is one function per message signature, and nothing here decides which messages may be
/// sent: the allowlist of selectors, the classes they may be sent to, and the facts their
/// results become are Swift, in ReconNativeSupport. A client never supplies a selector.
typedef NS_ENUM(NSInteger, ReconSendStatus) {
    ReconSendStatusValue,
    /// The object does not implement the selector.
    ReconSendStatusUnavailable,
    /// The message raised, or the object refused it.
    ReconSendStatusFailed,
};

ReconSendStatus ReconSendObject(id target, SEL selector, id _Nullable *_Nonnull result);
/// A string the target answered: verified against the runtime's class chain and copied here,
/// so the caller only ever holds a string this process made. A target that reports more
/// characters than `limit` is refused rather than copied.
ReconSendStatus ReconSendString(id target, SEL selector, NSUInteger limit,
                                NSString *_Nullable *_Nonnull result);
/// A snapshot of a collection the target answered. A collection that reports more elements
/// than `limit` is refused rather than reserved for: the target names the number, so the
/// target must not decide an allocation.
ReconSendStatus ReconCopyElements(id collection, NSUInteger limit,
                                  NSArray *_Nullable *_Nonnull result);
ReconSendStatus ReconSendObjectTo(id target, SEL selector, id _Nullable argument,
                                  id _Nullable *_Nonnull result);
ReconSendStatus ReconSendObjectAt(id target, SEL selector, NSInteger argument,
                                  id _Nullable *_Nonnull result);
/// A Core Foundation result, which the caller takes unretained.
ReconSendStatus ReconSendReference(id target, SEL selector,
                                   const void *_Nullable *_Nonnull result);

ReconSendStatus ReconSendBool(id target, SEL selector, BOOL *result);
/// `respondsToSelector:`, which an application may answer for itself.
ReconSendStatus ReconSendResponds(id target, SEL selector, SEL question, BOOL *result);

ReconSendStatus ReconSendInteger(id target, SEL selector, NSInteger *result);
ReconSendStatus ReconSendDouble(id target, SEL selector, double *result);
ReconSendStatus ReconSendFloat(id target, SEL selector, float *result);
ReconSendStatus ReconSendFloatFor(id target, SEL selector, NSInteger argument, float *result);

ReconSendStatus ReconSendRect(id target, SEL selector, CGRect *result);
ReconSendStatus ReconSendRectOf(id target, SEL selector, CGRect rect, CGRect *result);
ReconSendStatus ReconSendRectOfIn(id target, SEL selector, CGRect rect, id _Nullable view,
                                  CGRect *result);
ReconSendStatus ReconSendSize(id target, SEL selector, CGSize *result);
ReconSendStatus ReconSendTransform(id target, SEL selector, CATransform3D *result);

/// `attribute:atIndex:longestEffectiveRange:inRange:`, the one read that reports a range.
ReconSendStatus ReconSendAttribute(id target, SEL selector, id name, NSUInteger index,
                                   NSRange limit, NSRange *effective,
                                   id _Nullable *_Nonnull result);

NS_ASSUME_NONNULL_END
