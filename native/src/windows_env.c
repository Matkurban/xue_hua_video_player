#include "xhvp_internal.h"

#if defined(_WIN32)

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <windows.h>

static void xhvp_setenv_copy(const char *key, const char *value) {
  if (!key || !value) {
    return;
  }
  /* Process env for GStreamer/GIO in this process. */
  (void)_putenv_s(key, value);
  SetEnvironmentVariableA(key, value);
}

static int xhvp_dir_exists(const char *path) {
  DWORD attrs = GetFileAttributesA(path);
  return attrs != INVALID_FILE_ATTRIBUTES &&
         (attrs & FILE_ATTRIBUTE_DIRECTORY) != 0;
}

void xhvp_setup_windows_env(void) {
  const char *root = getenv("GSTREAMER_1_0_ROOT_MSVC_X86_64");
  char root_buf[MAX_PATH];
  if (!root || root[0] == '\0') {
    snprintf(root_buf, sizeof(root_buf), "C:\\gstreamer\\1.0\\msvc_x86_64");
    root = root_buf;
  }

  char plugins[MAX_PATH];
  char gio[MAX_PATH];
  snprintf(plugins, sizeof(plugins), "%s\\lib\\gstreamer-1.0", root);
  snprintf(gio, sizeof(gio), "%s\\lib\\gio\\modules", root);

  if (xhvp_dir_exists(plugins)) {
    xhvp_setenv_copy("GST_PLUGIN_SYSTEM_PATH", plugins);
  }
  if (xhvp_dir_exists(gio)) {
    xhvp_setenv_copy("GIO_MODULE_DIR", gio);
  }
}

#endif /* _WIN32 */
