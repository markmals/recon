#import <Foundation/Foundation.h>
NS_ASSUME_NONNULL_BEGIN

extern NSString *const ReconHelperBuild;
extern NSUInteger const ReconProtocolVersion;

@interface ReconRegistry : NSObject
- (nullable NSString *)handleForObject:(id)object;
- (nullable id)objectForHandle:(NSString *)handle;
- (void)revoke;
@end

BOOL ReconValidBounds(NSDictionary *request);
uint32_t ReconDecodeLength(const uint8_t * _Nonnull bytes);
void ReconEncodeLength(uint32_t length, uint8_t * _Nonnull bytes);
BOOL ReconValidFrameLength(uint32_t length);
NSString *ReconJSONString(NSString *value, NSUInteger maxScalars);
NSDictionary *ReconError(NSString *code, NSString *stage, NSString *message);
NSString * _Nullable ReconToken(void);
NSString * _Nullable ReconKind(id object);
BOOL ReconValidHandle(id handle);
NSDictionary *ReconProcessIdentity(void);
NS_ASSUME_NONNULL_END
