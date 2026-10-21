#import <UIKit/UIKit.h>
#import <objc/runtime.h>
#include "bindings/bindings.h"

// Winit on iOS (legacy AppDelegate launch) doesn't report the real window bounds, so Rust
// reads them from here. UIKit calls must happen on the main thread; Rust ensures that
// with `IosMainThread` in src/bevy/app.rs.

extern "C" void ios_screen_bounds(float* w, float* h, float* scale) {
    UIScreen* screen = [UIScreen mainScreen];
    CGRect b = [screen bounds];
    *w = (float)b.size.width;
    *h = (float)b.size.height;
    *scale = (float)[screen scale];
}

// The key window, or during early launch the only window. nil if there's none yet.
static UIWindow* _key_window(void) {
    UIApplication* app = [UIApplication sharedApplication];
    if (!app) return nil;
    NSArray* scenes = [[app connectedScenes] allObjects];
    for (UIScene* s in scenes) {
        if (![s isKindOfClass:[UIWindowScene class]]) continue;
        UIWindowScene* ws = (UIWindowScene*)s;
        for (UIWindow* w in [ws windows]) {
            if ([w isKeyWindow] || [[ws windows] count] == 1) return w;
        }
    }
    return nil;
}

extern "C" void ios_safe_area_insets(float* top, float* bottom,
                                     float* left, float* right) {
    *top = 0.0f; *bottom = 0.0f; *left = 0.0f; *right = 0.0f;
    UIWindow* w = _key_window();
    if (!w) return;
    UIEdgeInsets i = [w safeAreaInsets];
    *top = (float)i.top;
    *bottom = (float)i.bottom;
    *left = (float)i.left;
    *right = (float)i.right;
}

// ---------------------------------------------------------------------------
// URL-scheme deep-link handling
// ---------------------------------------------------------------------------
// iOS hands `kvint://join/<id>` URLs to the app delegate. They're queued here,
// and Rust drains them with `ios_poll_url`.

static NSMutableArray<NSString*>* g_pending_urls = nil;
static NSLock* g_url_lock = nil;

static void _ensure_url_storage(void) {
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        g_pending_urls = [[NSMutableArray alloc] init];
        g_url_lock = [[NSLock alloc] init];
    });
}

static void _store_url(NSURL* url) {
    if (!url) return;
    NSString* str = [url absoluteString];
    if (!str) return;
    _ensure_url_storage();
    [g_url_lock lock];
    [g_pending_urls addObject:str];
    [g_url_lock unlock];
}

// winit's default delegate doesn't implement openURL, so `main` installs this one
// once UIApplication exists. UIApplication holds it weakly, so it's also kept in a static.
@interface KvintAppDelegate : NSObject <UIApplicationDelegate>
@end

@implementation KvintAppDelegate

- (BOOL)application:(UIApplication*)app
            openURL:(NSURL*)url
            options:(NSDictionary<UIApplicationOpenURLOptionsKey, id>*)options {
    NSLog(@"kvint: application:openURL: fired with %@", url);
    _store_url(url);
    return YES;
}

@end

static KvintAppDelegate* g_kvint_delegate = nil;

extern "C" void ios_register_url_handler(void) {
    _ensure_url_storage();
}

extern "C" int ios_poll_url(char* buf, int buf_len) {
    if (!buf || buf_len <= 0) return 0;
    _ensure_url_storage();
    [g_url_lock lock];
    NSString* str = [g_pending_urls firstObject];
    if (str) {
        [g_pending_urls removeObjectAtIndex:0];
    }
    [g_url_lock unlock];
    if (!str) return 0;
    const char* utf8 = [str UTF8String];
    int len = (int)strlen(utf8);
    if (len > buf_len - 1) len = buf_len - 1;
    memcpy(buf, utf8, len);
    buf[len] = 0;
    return len;
}

// ---------------------------------------------------------------------------
// Clipboard (iOS UIPasteboard)
// ---------------------------------------------------------------------------

extern "C" int ios_clipboard_read(char* buf, int buf_len) {
    if (!buf || buf_len <= 0) return 0;
    UIPasteboard* pb = [UIPasteboard generalPasteboard];
    NSString* str = [pb string];
    if (!str) return 0;
    const char* utf8 = [str UTF8String];
    if (!utf8) return 0;
    int len = (int)strlen(utf8);
    if (len > buf_len - 1) len = buf_len - 1;
    memcpy(buf, utf8, len);
    buf[len] = 0;
    return len;
}

// The app's Documents directory, where settings are saved.
extern "C" int ios_documents_path(char* buf, int buf_len) {
    if (!buf || buf_len <= 0) return 0;
    NSArray* paths = NSSearchPathForDirectoriesInDomains(
        NSDocumentDirectory, NSUserDomainMask, YES);
    if ([paths count] == 0) return 0;
    NSString* dir = [paths firstObject];
    const char* utf8 = [dir UTF8String];
    if (!utf8) return 0;
    int len = (int)strlen(utf8);
    if (len > buf_len - 1) len = buf_len - 1;
    memcpy(buf, utf8, len);
    buf[len] = 0;
    return len;
}

extern "C" void ios_clipboard_write(const char* text) {
    if (!text) return;
    NSString* str = [NSString stringWithUTF8String:text];
    if (!str) return;
    [[UIPasteboard generalPasteboard] setString:str];
}

// ---------------------------------------------------------------------------
// Text input bridge (UITextField → on-screen keyboard)
// ---------------------------------------------------------------------------
// Bevy has no on-screen keyboard on iOS, so a hidden UITextField handles text input.
// Rust calls show, polls the text each frame, checks `dismissed` for Done, and calls hide.
// UIKit work is dispatched to the main thread; a locked copy of the text makes polling thread-safe.

static UITextField* g_text_field = nil;     // main-thread only
static id           g_text_delegate = nil;  // strong ref to delegate
static NSLock*      g_text_lock = nil;
static NSString*    g_text_shadow = @"";    // guarded by g_text_lock
static BOOL         g_text_dismissed = NO;  // guarded by g_text_lock

static void _ensure_text_lock(void) {
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        g_text_lock = [[NSLock alloc] init];
        g_text_shadow = @"";
    });
}

// Copies the field's text into the locked copy. Called on the main thread.
static void _update_shadow(UITextField* textField) {
    [g_text_lock lock];
    g_text_shadow = [(textField.text ?: @"") copy];
    [g_text_lock unlock];
}

@interface KvintTextInput : NSObject <UITextFieldDelegate>
@end

@implementation KvintTextInput
- (BOOL)textFieldShouldReturn:(UITextField*)textField {
    [textField resignFirstResponder];
    _update_shadow(textField);
    [g_text_lock lock];
    g_text_dismissed = YES;
    [g_text_lock unlock];
    return YES;
}
- (void)textChanged:(UITextField*)textField {
    _update_shadow(textField);
}
@end

// Creates the field if needed. Main thread only.
static void _ensure_text_field_main(void) {
    if (g_text_field) return;
    g_text_delegate = [[KvintTextInput alloc] init];
    UITextField* tf = [[UITextField alloc]
        initWithFrame:CGRectMake(-1000.0, -1000.0, 200.0, 30.0)];
    tf.delegate = (id<UITextFieldDelegate>)g_text_delegate;
    tf.autocorrectionType = UITextAutocorrectionTypeNo;
    tf.autocapitalizationType = UITextAutocapitalizationTypeNone;
    tf.spellCheckingType = UITextSpellCheckingTypeNo;
    tf.returnKeyType = UIReturnKeyDone;
    tf.hidden = YES;
    [tf addTarget:g_text_delegate
            action:@selector(textChanged:)
   forControlEvents:UIControlEventEditingChanged];
    g_text_field = tf;
}

extern "C" void ios_text_input_show(const char* initial) {
    _ensure_text_lock();
    NSString* s = (initial ? [NSString stringWithUTF8String:initial] : nil) ?: @"";

    // Seed the copy so an early poll returns the initial text.
    [g_text_lock lock];
    g_text_shadow = s;
    g_text_dismissed = NO;
    [g_text_lock unlock];

    dispatch_async(dispatch_get_main_queue(), ^{
        _ensure_text_field_main();
        UIWindow* win = _key_window();
        if (win && g_text_field.superview != win) {
            [win addSubview:g_text_field];
        }
        g_text_field.text = s;
        [g_text_field becomeFirstResponder];
    });
}

extern "C" void ios_text_input_hide(void) {
    dispatch_async(dispatch_get_main_queue(), ^{
        if (g_text_field) {
            [g_text_field resignFirstResponder];
        }
    });
}

extern "C" int ios_text_input_poll(char* buf, int buf_len) {
    if (!buf || buf_len <= 0) return 0;
    _ensure_text_lock();
    [g_text_lock lock];
    NSString* s = g_text_shadow ? [g_text_shadow copy] : @"";
    [g_text_lock unlock];
    const char* utf8 = [s UTF8String];
    if (!utf8) {
        buf[0] = 0;
        return 0;
    }
    int len = (int)strlen(utf8);
    if (len > buf_len - 1) len = buf_len - 1;
    memcpy(buf, utf8, len);
    buf[len] = 0;
    return len;
}

extern "C" int ios_text_input_dismissed(void) {
    _ensure_text_lock();
    [g_text_lock lock];
    BOOL d = g_text_dismissed;
    g_text_dismissed = NO;
    [g_text_lock unlock];
    return d ? 1 : 0;
}

int main(int argc, char * argv[]) {
    @autoreleasepool {
        _ensure_url_storage();

        // Once UIApplication exists: queue the cold-launch URL (it arrives in launchOptions,
        // not openURL), and install our delegate so URLs opened while running reach us.
        [[NSNotificationCenter defaultCenter]
            addObserverForName:UIApplicationDidFinishLaunchingNotification
                        object:nil
                         queue:nil
                    usingBlock:^(NSNotification* note) {
            NSDictionary* opts = [note userInfo];
            NSURL* url = opts ? opts[UIApplicationLaunchOptionsURLKey] : nil;
            NSLog(@"kvint: didFinishLaunching, launchURL=%@", url);
            if (url) _store_url(url);

            UIApplication* app = [UIApplication sharedApplication];
            if (app) {
                g_kvint_delegate = [[KvintAppDelegate alloc] init];
                [app setDelegate:g_kvint_delegate];
                NSLog(@"kvint: installed KvintAppDelegate as UIApplication.delegate");
            }
        }];
    }

    ffi::start_app();
    return 0;
}
