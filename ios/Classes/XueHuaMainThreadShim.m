#import <CoreFoundation/CoreFoundation.h>
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

/// Returns true when host has non-zero bounds and sinkLayer is in the host hierarchy.
/// On success, balances +1 retain from Rust read_sink_layer via CFRelease.
/// On failure (zero bounds, null args), does not CFRelease — caller must release.
static bool xhvp_attach_layer_on_main(uintptr_t host_view, uintptr_t layer) {
  if (host_view == 0 || layer == 0) {
    return false;
  }
  UIView *host = (__bridge UIView *)(void *)host_view;
  if (host.bounds.size.width <= 0.0 || host.bounds.size.height <= 0.0) {
    return false;
  }
  CALayer *sinkLayer = (__bridge CALayer *)(void *)layer;
  if (host.layer.sublayers == nil
      || ![host.layer.sublayers containsObject:sinkLayer]) {
    sinkLayer.frame = host.bounds;
    sinkLayer.contentsScale = host.layer.contentsScale;
    [host.layer addSublayer:sinkLayer];
  } else {
    sinkLayer.frame = host.bounds;
    sinkLayer.contentsScale = host.layer.contentsScale;
  }
  if (host.layer.sublayers == nil
      || ![host.layer.sublayers containsObject:sinkLayer]) {
    return false;
  }
  CFRelease((CFTypeRef)(void *)layer);
  return true;
}

void xhvp_ios_attach_layer_to_host(uintptr_t host_view, uintptr_t layer) {
  if (host_view == 0 || layer == 0) {
    return;
  }
  void (^attach)(void) = ^{
    (void)xhvp_attach_layer_on_main(host_view, layer);
  };
  if ([NSThread isMainThread]) {
    attach();
  } else {
    dispatch_async(dispatch_get_main_queue(), attach);
  }
}

bool xhvp_ios_attach_layer_to_host_sync(uintptr_t host_view, uintptr_t layer) {
  if (host_view == 0 || layer == 0) {
    return false;
  }
  __block bool result = false;
  void (^attach)(void) = ^{
    result = xhvp_attach_layer_on_main(host_view, layer);
  };
  if ([NSThread isMainThread]) {
    attach();
  } else {
    dispatch_sync(dispatch_get_main_queue(), attach);
  }
  return result;
}

/// Returns true when host UIView has non-zero bounds (safe from any thread).
bool xhvp_ios_host_view_has_bounds(uintptr_t host_view) {
  if (host_view == 0) {
    return false;
  }
  __block bool result = false;
  void (^check)(void) = ^{
    UIView *host = (__bridge UIView *)(void *)host_view;
    result =
        host.bounds.size.width > 0.0 && host.bounds.size.height > 0.0;
  };
  if ([NSThread isMainThread]) {
    check();
  } else {
    dispatch_sync(dispatch_get_main_queue(), check);
  }
  return result;
}

void xhvp_ios_attach_layer_to_host_async(uintptr_t host_view, uintptr_t layer,
                                         void (*complete_fn)(bool, void *),
                                         void *complete_ctx) {
  if (host_view == 0 || layer == 0) {
    if (complete_fn != NULL) {
      complete_fn(false, complete_ctx);
    }
    return;
  }
  void (^attach)(void) = ^{
    bool ok = xhvp_attach_layer_on_main(host_view, layer);
    if (complete_fn != NULL) {
      complete_fn(ok, complete_ctx);
    }
  };
  if ([NSThread isMainThread]) {
    attach();
  } else {
    dispatch_async(dispatch_get_main_queue(), attach);
  }
}
