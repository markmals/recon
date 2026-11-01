#import "ReconMessageSend.h"

#import <objc/message.h>

#if !__has_feature(objc_arc)
#error "ReconNativeAccessors requires ARC"
#endif

/// One send, and the check that the object implements it, inside one `@try`. Both are messages
/// to the application's own object, so both belong on this side of the boundary.
#define RECON_SEND(CALL)                                                       \
    @try {                                                                     \
        if (![target respondsToSelector:selector]) {                           \
            return ReconSendStatusUnavailable;                                 \
        }                                                                      \
        CALL;                                                                  \
        return ReconSendStatusValue;                                           \
    } @catch (id exception) {                                                  \
        return ReconSendStatusFailed;                                          \
    }

ReconSendStatus ReconSendObject(id target, SEL selector, id *result) {
    *result = nil;
    RECON_SEND(*result = ((id (*)(id, SEL))objc_msgSend)(target, selector))
}

ReconSendStatus ReconSendObjectTo(id target, SEL selector, id argument, id *result) {
    *result = nil;
    RECON_SEND(*result = ((id (*)(id, SEL, id))objc_msgSend)(target, selector, argument))
}

ReconSendStatus ReconSendObjectAt(id target, SEL selector, NSInteger argument, id *result) {
    *result = nil;
    RECON_SEND(*result = ((id (*)(id, SEL, NSInteger))objc_msgSend)(target, selector, argument))
}

ReconSendStatus ReconSendReference(id target, SEL selector, const void **result) {
    *result = NULL;
    RECON_SEND(*result = ((const void *(*)(id, SEL))objc_msgSend)(target, selector))
}

ReconSendStatus ReconSendBool(id target, SEL selector, BOOL *result) {
    *result = NO;
    RECON_SEND(*result = ((BOOL (*)(id, SEL))objc_msgSend)(target, selector))
}

ReconSendStatus ReconSendResponds(id target, SEL selector, SEL question, BOOL *result) {
    *result = NO;
    RECON_SEND(*result = ((BOOL (*)(id, SEL, SEL))objc_msgSend)(target, selector, question))
}

ReconSendStatus ReconSendInteger(id target, SEL selector, NSInteger *result) {
    *result = 0;
    RECON_SEND(*result = ((NSInteger (*)(id, SEL))objc_msgSend)(target, selector))
}

ReconSendStatus ReconSendDouble(id target, SEL selector, double *result) {
    *result = 0;
    RECON_SEND(*result = ((double (*)(id, SEL))objc_msgSend)(target, selector))
}

ReconSendStatus ReconSendFloat(id target, SEL selector, float *result) {
    *result = 0;
    RECON_SEND(*result = ((float (*)(id, SEL))objc_msgSend)(target, selector))
}

ReconSendStatus ReconSendFloatFor(id target, SEL selector, NSInteger argument, float *result) {
    *result = 0;
    RECON_SEND(*result = ((float (*)(id, SEL, NSInteger))objc_msgSend)(target, selector, argument))
}

ReconSendStatus ReconSendRect(id target, SEL selector, CGRect *result) {
    *result = CGRectZero;
    RECON_SEND(*result = ((CGRect (*)(id, SEL))objc_msgSend)(target, selector))
}

ReconSendStatus ReconSendRectOf(id target, SEL selector, CGRect rect, CGRect *result) {
    *result = CGRectZero;
    RECON_SEND(*result = ((CGRect (*)(id, SEL, CGRect))objc_msgSend)(target, selector, rect))
}

ReconSendStatus ReconSendRectOfIn(id target, SEL selector, CGRect rect, id view, CGRect *result) {
    *result = CGRectZero;
    RECON_SEND(
        *result = ((CGRect (*)(id, SEL, CGRect, id))objc_msgSend)(target, selector, rect, view))
}

ReconSendStatus ReconSendSize(id target, SEL selector, CGSize *result) {
    *result = CGSizeZero;
    RECON_SEND(*result = ((CGSize (*)(id, SEL))objc_msgSend)(target, selector))
}

ReconSendStatus ReconSendTransform(id target, SEL selector, CATransform3D *result) {
    *result = (CATransform3D){0};
    RECON_SEND(*result = ((CATransform3D (*)(id, SEL))objc_msgSend)(target, selector))
}

ReconSendStatus ReconSendAttribute(id target, SEL selector, id name, NSUInteger index,
                                   NSRange limit, NSRange *effective, id *result) {
    *result = nil;
    RECON_SEND(
        *result = ((id (*)(id, SEL, id, NSUInteger, NSRange *, NSRange))objc_msgSend)(
            target, selector, name, index, effective, limit))
}
