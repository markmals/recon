#import "../ReconCore.h"
#import <Foundation/Foundation.h>
#include <assert.h>

@interface TestObject : NSObject @end
@implementation TestObject @end

static void testBounds(void) {
    for (NSInteger d = -2; d <= 66; d++) {
        for (NSInteger n = -1; n <= 25002; n += 317) {
            BOOL valid = d >= 0 && d <= 64 && n >= 1 && n <= 25000;
            assert(ReconValidBounds(@{ @"depth": @(d), @"limit": @(n) }) == valid);
        }
    }
    assert(ReconValidBounds(@{}));
    assert(!ReconValidBounds(@{ @"depth": @1.5 }));
    assert(!ReconValidBounds(@{ @"limit": @"2" }));
    assert(!ReconValidBounds(@{ @"fields": @[ @"basic", @"basic" ] }));
    assert(!ReconValidBounds(@{ @"fields": @[] }));
    assert(!ReconValidBounds(@{ @"fields": @[ @"unknown" ] }));
    assert(ReconValidBounds(@{ @"fields": @[ @"basic", @"font" ] }));
}

static void testRegistry(void) {
    ReconRegistry *registry = [ReconRegistry new];
    __block NSString *first;
    @autoreleasepool {
        TestObject *object = [TestObject new];
        first = [registry handleForObject:object];
        assert(first.length == 16);
        assert([first rangeOfCharacterFromSet:[NSCharacterSet characterSetWithCharactersInString:@"0123456789abcdef"].invertedSet].location == NSNotFound);
        assert(ReconValidHandle(first));
        assert(ReconValidHandle(@"abcdef1234567890"));
        assert(!ReconValidHandle(@"ABCDEF1234567890"));
        assert(!ReconValidHandle([first stringByAppendingString:@"0"]));
        assert(!ReconValidHandle(@"n:invalid"));
        assert([first isEqualToString:[registry handleForObject:object]]);
        assert([registry objectForHandle:first] == object);
    }
    assert([registry objectForHandle:first] == nil);
    TestObject *replacement = [TestObject new];
    NSString *second = [registry handleForObject:replacement];
    assert(![first isEqualToString:second]);
    assert([registry objectForHandle:first] == nil);
    [registry revoke];
    assert([registry objectForHandle:second] == nil);
    ReconRegistry *nextSession = [ReconRegistry new];
    assert(![[nextSession handleForObject:replacement] isEqualToString:second]);
}

static void testRegistryCapacity(void) {
    ReconRegistry *registry = [ReconRegistry new];
    NSMutableArray *live = [NSMutableArray arrayWithCapacity:100000];
    __weak TestObject *removed = nil;
    TestObject *overflow = [TestObject new];
    @autoreleasepool {
        for (NSUInteger i = 0; i < 100000; i++) {
            TestObject *object = [TestObject new];
            if (i == 0) removed = object;
            [live addObject:object];
            assert([registry handleForObject:object] != nil);
        }
        assert([registry handleForObject:overflow] == nil);
    }
    @autoreleasepool { [live removeObjectAtIndex:0]; }
    assert(removed == nil);
    assert([registry handleForObject:overflow] != nil);
}

static void testFrames(void) {
    uint8_t b[4];
    for (uint32_t size = 0; size <= 65538; size++) {
        ReconEncodeLength(size, b);
        assert(ReconDecodeLength(b) == size);
        assert(ReconValidFrameLength(size) == (size > 0 && size <= 65536));
    }
    assert([ReconJSONString(@"abc", 4096) isEqualToString:@"abc"]);
    NSMutableString *longString = [NSMutableString new];
    for (NSUInteger i = 0; i < 4100; i++) [longString appendString:@"😀"];
    assert([ReconJSONString(longString, 4096) length] == 8192);
}

int main(void) {
    @autoreleasepool { testBounds(); testRegistry(); testRegistryCapacity(); testFrames(); }
    return 0;
}
