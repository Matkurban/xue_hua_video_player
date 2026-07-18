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

static int xhvp_file_exists(const char *path) {
  DWORD attrs = GetFileAttributesA(path);
  return attrs != INVALID_FILE_ATTRIBUTES &&
         (attrs & FILE_ATTRIBUTE_DIRECTORY) == 0;
}

static void xhvp_copy_file_best_effort(const char *src, const char *dst) {
  if (!src || !dst || !xhvp_file_exists(src) || xhvp_file_exists(dst)) {
    return;
  }
  (void)CopyFileA(src, dst, TRUE);
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
  char seed[MAX_PATH];
  snprintf(plugins, sizeof(plugins), "%s\\lib\\gstreamer-1.0", root);
  snprintf(gio, sizeof(gio), "%s\\lib\\gio\\modules", root);
  snprintf(seed, sizeof(seed), "%s\\lib\\gstreamer-registry.bin.seed", root);

  if (xhvp_dir_exists(plugins)) {
    xhvp_setenv_copy("GST_PLUGIN_SYSTEM_PATH", plugins);
  }
  if (xhvp_dir_exists(gio)) {
    xhvp_setenv_copy("GIO_MODULE_DIR", gio);
  }

  /* Persist registry under LOCALAPPDATA (or TEMP) to avoid rescans. */
  char registry[MAX_PATH];
  const char *local = getenv("LOCALAPPDATA");
  if (local && local[0] != '\0') {
    char dir[MAX_PATH];
    snprintf(dir, sizeof(dir), "%s\\xue_hua_video_player", local);
    CreateDirectoryA(dir, NULL);
    snprintf(registry, sizeof(registry), "%s\\gstreamer-registry.bin", dir);
  } else {
    const char *tmp = getenv("TEMP");
    if (!tmp || tmp[0] == '\0') {
      tmp = "C:\\Temp";
    }
    snprintf(registry, sizeof(registry), "%s\\gstreamer-registry.bin", tmp);
  }
  xhvp_setenv_copy("GST_REGISTRY_FORK", "no");
  xhvp_setenv_copy("GST_REGISTRY", registry);
  xhvp_copy_file_best_effort(seed, registry);
}

#endif /* _WIN32 */
