#import <AVFoundation/AVFoundation.h>
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

/// Releases a +1 CFRetain (from Rust read_sink_layer) on the main thread.
/// AVSampleBufferDisplayLayer must only be deallocated on the main thread.
void xhvp_ios_release_layer_main(uintptr_t layer) {
  if (layer == 0) {
    return;
  }
  void (^release)(void) = ^{
    CFRelease((CFTypeRef)(void *)layer);
  };
  if ([NSThread isMainThread]) {
    release();
  } else {
    dispatch_async(dispatch_get_main_queue(), release);
  }
}

/// Removes the GStreamer sink CALayer(s) from the host view on the main thread.
/// Called on shell teardown / rebuild so a stale display layer (whose owning
/// sink is being freed) cannot outlive the sink in the render tree.
void xhvp_ios_detach_sink_layers(uintptr_t host_view) {
  if (host_view == 0) {
    return;
  }
  void (^detach)(void) = ^{
    UIView *host = (__bridge UIView *)(void *)host_view;
    NSArray<CALayer *> *sublayers = [host.layer.sublayers copy];
    for (CALayer *sublayer in sublayers) {
      [sublayer removeFromSuperlayer];
    }
  };
  if ([NSThread isMainThread]) {
    detach();
  } else {
    dispatch_async(dispatch_get_main_queue(), detach);
  }
}

/// Reads the AVSampleBufferDisplayLayer status/error on the main thread (diag).
/// `out_status`: 0 unknown, 1 rendering, 2 failed, -1 not a display layer.
void xhvp_ios_layer_status(uintptr_t layer, int32_t *out_status,
                           int32_t *out_error_code) {
  int32_t status = -1;
  int32_t error_code = 0;
  if (layer != 0) {
    CALayer *raw = (__bridge CALayer *)(void *)layer;
    if ([raw isKindOfClass:[AVSampleBufferDisplayLayer class]]) {
      AVSampleBufferDisplayLayer *display = (AVSampleBufferDisplayLayer *)raw;
      __block AVQueuedSampleBufferRenderingStatus s;
      __block NSError *err;
      void (^read)(void) = ^{
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
        s = display.status;
        err = display.error;
#pragma clang diagnostic pop
      };
      if ([NSThread isMainThread]) {
        read();
      } else {
        dispatch_sync(dispatch_get_main_queue(), read);
      }
      status = (int32_t)s;
      error_code = err != nil ? (int32_t)err.code : 0;
    }
  }
  if (out_status != NULL) {
    *out_status = status;
  }
  if (out_error_code != NULL) {
    *out_error_code = error_code;
  }
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
