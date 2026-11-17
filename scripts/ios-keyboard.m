// Local simulator test tool. Private simulator API signatures were checked against
// Meta idb v1.1.7 (MIT): FBSimulatorControl/HID and PrivateHeaders/CoreSimulator.
#import <Foundation/Foundation.h>
#import <dlfcn.h>

@interface NSObject (CarlosSimulatorAPI)
+ (id)sharedServiceContextForDeveloperDir:(NSString *)path error:(NSError **)error;
- (id)defaultDeviceSetWithError:(NSError **)error;
- (NSArray *)availableDevices;
- (NSUUID *)UDID;
- (NSString *)name;
- (id)initWithDevice:(id)device error:(NSError **)error;
- (void)sendWithMessage:(void *)message freeWhenDone:(BOOL)freeWhenDone completionQueue:(dispatch_queue_t)queue completion:(void (^)(NSError *))completion;
@end

int main(int argc, const char *argv[]) { @autoreleasepool {
    if (argc < 3) return 2;
    void *core = dlopen("/Library/Developer/PrivateFrameworks/CoreSimulator.framework/CoreSimulator", RTLD_NOW | RTLD_GLOBAL);
    void *kit = dlopen("/Applications/Xcode.app/Contents/Developer/Library/PrivateFrameworks/SimulatorKit.framework/SimulatorKit", RTLD_NOW | RTLD_GLOBAL);
    if (!core || !kit) { fprintf(stderr, "Simulator frameworks could not load: %s\n", dlerror()); return 3; }
    void *(*makeKey)(int, int) = dlsym(kit, "IndigoHIDMessageForKeyboardArbitrary");
    Class service = NSClassFromString(@"SimServiceContext");
    Class hidClass = NSClassFromString(@"SimulatorKit.SimDeviceLegacyHIDClient");
    if (!makeKey || !service || !hidClass) { fprintf(stderr, "Simulator HID API is unavailable\n"); return 4; }
    NSError *error = nil;
    id context = [service sharedServiceContextForDeveloperDir:@"/Applications/Xcode.app/Contents/Developer" error:&error];
    id devices = [context defaultDeviceSetWithError:&error];
    NSString *uuid = [NSString stringWithUTF8String:argv[1]];
    id selected = nil;
    for (id device in [devices availableDevices]) {
        if ([[device UDID].UUIDString isEqualToString:uuid] && [[device name] hasPrefix:@"carlos "]) { selected = device; break; }
    }
    if (!selected) { fprintf(stderr, "Expected one owned Carlos simulator\n"); return 5; }
    if (!strcmp(argv[2], "probe")) { puts("Owned simulator and raw keyboard HID API are available"); return 0; }
    id hid = [[hidClass alloc] initWithDevice:selected error:&error];
    if (!hid) { fprintf(stderr, "Could not attach owned HID client\n"); return 6; }
    int modifier = !strcmp(argv[2], "shift") ? 225 : !strcmp(argv[2], "option") ? 226 : 0;
    BOOL printable = !strcmp(argv[2], "x");
    if (strcmp(argv[2], "plain") && !modifier && !printable) return 2;
    int key = printable ? 27 : 40;
    NSArray<NSNumber *> *keys = @[@(modifier), @(key), @(key), @(modifier)];
    __block NSUInteger cursor = 0;
    // One process sends one sequence; the static block keeps it alive until exit.
    static void (^deliver)(void);
    deliver = ^{
        while (cursor < keys.count && keys[cursor].intValue == 0) cursor += 1;
        if (cursor == keys.count) { puts("Sent raw keyboard press and release"); exit(0); }
        NSUInteger index = cursor++;
        void *message = makeKey(keys[index].intValue, index < 2 ? 1 : 2);
        if (!message) exit(7);
        [hid sendWithMessage:message freeWhenDone:YES completionQueue:dispatch_get_main_queue() completion:^(NSError *failure) {
            if (failure) { fprintf(stderr, "Simulator rejected the keyboard event\n"); exit(8); }
            // Transport acknowledgement does not mean UIKit consumed the event.
            // Yield the native run loop with a brief physical key hold, including modifiers.
            dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 50 * NSEC_PER_MSEC), dispatch_get_main_queue(), deliver);
        }];
    };
    dispatch_async(dispatch_get_main_queue(), deliver);
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 12 * NSEC_PER_SEC), dispatch_get_main_queue(), ^{ exit(8); });
    dispatch_main();
} }
