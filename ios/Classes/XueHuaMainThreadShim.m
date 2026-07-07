#import <Foundation/Foundation.h>
#import <dispatch/dispatch.h>
#import <QuartzCore/QuartzCore.h>
#import <UIKit/UIKit.h>

void xhvp_dispatch_sync_main_fn(void (*fn)(void *), void *ctx) {
  if (fn == NULL) {
    return;
  }
  if ([NSThread isMainThread]) {
    fn(ctx);
  } else {
    dispatch_sync(dispatch_get_main_queue(), ^{
      fn(ctx);
    });
  }
}

void xhvp_ios_attach_layer_to_host(uintptr_t host_view, uintptr_t layer) {
  if (host_view == 0 || layer == 0) {
    return;
  }
  void (^attach)(void) = ^{
    UIView *host = (__bridge UIView *)(void *)host_view;
    CALayer *sinkLayer = (__bridge CALayer *)(void *)layer;
    if (host.layer.sublayers == nil
        || ![host.layer.sublayers containsObject:sinkLayer]) {
      sinkLayer.frame = host.bounds;
      sinkLayer.contentsScale = host.layer.contentsScale;
      [host.layer addSublayer:sinkLayer];
    } else {
      sinkLayer.frame = host.bounds;
    }
  };
  if ([NSThread isMainThread]) {
    attach();
  } else {
    dispatch_async(dispatch_get_main_queue(), attach);
  }
}
