#import <Foundation/Foundation.h>

/// The content hash of every input the helper was built from. A request that names a different
/// build is refused rather than answered from the wrong code.
extern NSString *const ReconHelperBuild;
extern NSUInteger const ReconProtocolVersion;

/// Length-prefixed framing: four big-endian bytes, then that many bytes of JSON.
uint32_t ReconDecodeLength(const uint8_t b[4]);
void ReconEncodeLength(uint32_t length, uint8_t b[4]);
BOOL ReconValidFrameLength(uint32_t length);

NSDictionary *ReconError(NSString *code, NSString *stage, NSString *message);
/// 32 bytes of secure randomness in hexadecimal, or nil when the system has none to give.
NSString *ReconToken(void);
