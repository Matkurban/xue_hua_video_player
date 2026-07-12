#include "xhvp_internal.h"

#if defined(__APPLE__)
#include <TargetConditionals.h>
#endif

#if defined(TARGET_OS_IPHONE) && TARGET_OS_IPHONE

/* GIO OpenSSL module entry points from the static iOS GStreamer.framework. */
extern void _g_io_modules_ensure_extension_points_registered(void);
extern void g_io_openssl_load(void *module);

void xhvp_register_ios_tls_backend(void) {
  _g_io_modules_ensure_extension_points_registered();
  g_io_openssl_load(NULL);
}

#endif
