#import <dispatch/dispatch.h>

void xhvp_macos_dispatch_sync_main(void (*fn)(void *), void *ctx) {
  if (fn == NULL) {
    return;
  }
  dispatch_sync(dispatch_get_main_queue(), ^{
    fn(ctx);
  });
}

void xhvp_macos_dispatch_async_main(void (*fn)(void *), void *ctx) {
  if (fn == NULL) {
    return;
  }
  dispatch_async(dispatch_get_main_queue(), ^{
    fn(ctx);
  });
}
