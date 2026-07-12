#include "xhvp_internal.h"

#if defined(__APPLE__)

#include <TargetConditionals.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#if !TARGET_OS_IPHONE
#include <libgen.h>
#include <mach-o/dyld.h>
#endif

static void xhvp_setenv_copy(const char *key, const char *value) {
  if (!key || !value) {
    return;
  }
  setenv(key, value, 1);
}

static void xhvp_setup_sandbox_writable_env(void) {
  const char *tmp = getenv("TMPDIR");
  if (!tmp || tmp[0] == '\0') {
    tmp = "/tmp";
  }

  /* Prefer the sandbox HOME iOS already provides (app container). Falling back
   * to TMPDIR's parent matches the old Rust setup_sandbox_writable_env. */
  const char *home = getenv("HOME");
  char home_buf[PATH_MAX];
  if (!home || home[0] == '\0') {
    const char *slash = strrchr(tmp, '/');
    if (slash && slash != tmp) {
      size_t n = (size_t)(slash - tmp);
      if (n >= sizeof(home_buf)) {
        n = sizeof(home_buf) - 1;
      }
      memcpy(home_buf, tmp, n);
      home_buf[n] = '\0';
    } else {
      snprintf(home_buf, sizeof(home_buf), "%s", tmp);
    }
    home = home_buf;
  }

  char cache_buf[PATH_MAX];
  char docs_buf[PATH_MAX];
  char registry[PATH_MAX];
  snprintf(cache_buf, sizeof(cache_buf), "%s/Library/Caches", home);
  snprintf(docs_buf, sizeof(docs_buf), "%s/Documents", home);
  snprintf(registry, sizeof(registry), "%s/gstreamer-registry.bin", tmp);

  /* Best-effort create dirs; ignore failures (sandbox may already have them). */
  {
    char lib_buf[PATH_MAX];
    snprintf(lib_buf, sizeof(lib_buf), "%s/Library", home);
    (void)mkdir(lib_buf, 0755);
  }
  (void)mkdir(cache_buf, 0755);
  (void)mkdir(docs_buf, 0755);

  xhvp_setenv_copy("ORC_CODE", "backup");
  xhvp_setenv_copy("HOME", home);
  xhvp_setenv_copy("TMPDIR", tmp);
  xhvp_setenv_copy("TMP", tmp);
  xhvp_setenv_copy("TEMP", tmp);
  xhvp_setenv_copy("XDG_CACHE_HOME", cache_buf);
  xhvp_setenv_copy("XDG_CONFIG_HOME", cache_buf);
  xhvp_setenv_copy("XDG_DATA_HOME", docs_buf);
  xhvp_setenv_copy("XDG_RUNTIME_DIR", tmp);
  xhvp_setenv_copy("XDG_DATA_DIRS", docs_buf);
  xhvp_setenv_copy("XDG_CONFIG_DIRS", docs_buf);
  xhvp_setenv_copy("FONTCONFIG_PATH", tmp);
  xhvp_setenv_copy("GST_REGISTRY", registry);
}

#if defined(TARGET_OS_IPHONE) && TARGET_OS_IPHONE

void xhvp_setup_ios_env(void) {
  xhvp_setup_sandbox_writable_env();

  /* Static iOS GStreamer has no on-disk plugin tree. An unset system path can
   * become NULL and trip g_dir_open_with_errno / g_filename_to_utf8. Empty
   * string disables scanning (see GStreamer "Running Applications" docs). */
  xhvp_setenv_copy("GST_PLUGIN_SYSTEM_PATH", "");
  xhvp_setenv_copy("GST_PLUGIN_SYSTEM_PATH_1_0", "");
  xhvp_setenv_copy("GST_PLUGIN_PATH", "");
  xhvp_setenv_copy("GST_PLUGIN_PATH_1_0", "");
  xhvp_setenv_copy("GIO_MODULE_DIR", "");

  xhvp_setenv_copy(
      "GST_PLUGIN_FEATURE_RANK",
      "vtdec:PRIMARY,vtdec_hw:PRIMARY,vtdemux:PRIMARY,"
      "avdec_h264:SECONDARY,avdec_h265:SECONDARY");
}

#else /* macOS */

static int xhvp_dir_exists(const char *path) {
  struct stat st;
  return path && stat(path, &st) == 0 && S_ISDIR(st.st_mode);
}

static int xhvp_find_bundled_gstreamer_lib(char *out, size_t out_len) {
  char exe[PATH_MAX];
  uint32_t size = sizeof(exe);
  if (_NSGetExecutablePath(exe, &size) != 0) {
    return 0;
  }
  char resolved[PATH_MAX];
  if (!realpath(exe, resolved)) {
    snprintf(resolved, sizeof(resolved), "%s", exe);
  }
  char *dir = dirname(resolved);
  /* …/Contents/MacOS -> …/Contents */
  char contents[PATH_MAX];
  snprintf(contents, sizeof(contents), "%s/..", dir);
  char contents_real[PATH_MAX];
  if (!realpath(contents, contents_real)) {
    snprintf(contents_real, sizeof(contents_real), "%s", contents);
  }
  snprintf(out, out_len,
           "%s/Frameworks/GStreamer.framework/Versions/1.0/lib", contents_real);
  if (xhvp_dir_exists(out)) {
    return 1;
  }
  snprintf(out, out_len,
           "/Library/Frameworks/GStreamer.framework/Versions/1.0/lib");
  return xhvp_dir_exists(out);
}

void xhvp_setup_macos_env(void) {
  xhvp_setup_sandbox_writable_env();

  char lib_dir[PATH_MAX];
  if (xhvp_find_bundled_gstreamer_lib(lib_dir, sizeof(lib_dir))) {
    char plugins[PATH_MAX];
    char gio[PATH_MAX];
    snprintf(plugins, sizeof(plugins), "%s/gstreamer-1.0", lib_dir);
    snprintf(gio, sizeof(gio), "%s/gio/modules", lib_dir);
    if (xhvp_dir_exists(plugins)) {
      xhvp_setenv_copy("GST_PLUGIN_SYSTEM_PATH", plugins);
    }
    if (xhvp_dir_exists(gio)) {
      xhvp_setenv_copy("GIO_MODULE_DIR", gio);
    }
  }

  xhvp_setenv_copy(
      "GST_PLUGIN_FEATURE_RANK",
      "vtdec:PRIMARY,vtdemux:PRIMARY,avdec_h264:SECONDARY,avdec_h265:SECONDARY");
}

#endif /* !TARGET_OS_IPHONE */

#endif /* __APPLE__ */
