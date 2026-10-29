#import "ReconCore.h"
#import <AppKit/AppKit.h>

@interface ReconWork : NSObject
@property (nonatomic, strong) ReconRegistry *registry;
@property (nonatomic, strong) NSDictionary *request;
@property (nonatomic, strong) NSMutableArray<NSString *> *narrowed;
@property (nonatomic, strong) NSMutableArray<NSString *> *truncated;
@property (nonatomic) NSUInteger visited;
@property (nonatomic) NSUInteger emitted;
@property (nonatomic) NSUInteger limit;
@property (nonatomic) NSUInteger depth;
@property (nonatomic) uint64_t deadline;
@property (nonatomic) NSTimeInterval startedAt;
@property (nonatomic, copy) NSString *sessionId;
- (BOOL)consume:(NSString *)reason;
- (BOOL)expired;
- (NSString *)reference:(id)object;
- (NSDictionary *)record:(id)object fields:(NSSet<NSString *> *)fields;
- (NSDictionary *)complete:(NSDictionary *)payload;
@end

NSDictionary *ReconInspect(ReconWork *work);
NSDictionary *ReconBasic(id object, ReconWork *work);
NSDictionary *ReconAppearance(id object, ReconWork *work);
NSDictionary *ReconFont(id object, ReconWork *work);
NSDictionary *ReconLayout(id object, ReconWork *work);
NSDictionary *ReconLayer(id object, ReconWork *work);
NSDictionary *ReconCell(id object, ReconWork *work);
NSDictionary *ReconFontDescriptor(NSFont *font, ReconWork *work);
NSString *ReconBounded(id value, ReconWork *work);
NSDictionary *ReconStatus(NSString *status);
NSDictionary *ReconRect(NSRect rect, NSString *space, NSString *orientation, ReconWork *work);
NSDictionary *ReconColor(NSColor *color, ReconWork *work);
