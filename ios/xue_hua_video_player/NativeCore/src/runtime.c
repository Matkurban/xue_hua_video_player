#include "xhvp_internal.h"

#include <stdio.h>
#include <string.h>

#if defined(__APPLE__)
#include <TargetConditionals.h>
#endif

#if defined(__ANDROID__)
#include <android/log.h>
#define XHVP_RUNTIME_LOGI(...)                                                 \
  __android_log_print(ANDROID_LOG_INFO, "XhvpNative", __VA_ARGS__)
#else
#define XHVP_RUNTIME_LOGI(...)                                                 \
  do {                                                                         \
    g_message(__VA_ARGS__);                                                    \
  } while (0)
#endif

static XhvpRuntime g_runtime;
static GOnce g_runtime_once = G_ONCE_INIT;

/* Serializes first-time runtime start across sync + async callers. */
static GMutex g_start_mu;
static GCond g_start_cond;
static gboolean g_start_mu_ready = FALSE;
static gboolean g_start_in_progress = FALSE;
static int32_t g_start_result = XHVP_ERR_OK;

static void xhvp_start_gate_ensure(void) {
  if (g_start_mu_ready) {
    return;
  }
  g_mutex_init(&g_start_mu);
  g_cond_init(&g_start_cond);
  g_start_mu_ready = TRUE;
}

static gpointer xhvp_runtime_players_init(gpointer data) {
  (void)data;
  g_mutex_init(&g_runtime.players_mu);
  g_runtime.next_id = 1;
  for (int i = 0; i < XHVP_MAX_PLAYERS; i++) {
    g_runtime.players[i].id = 0;
    g_runtime.players[i].in_use = false;
    g_mutex_init(&g_runtime.players[i].frame_mu);
  }
  xhvp_start_gate_ensure();
  return NULL;
}

XhvpRuntime *xhvp_runtime(void) {
  g_once(&g_runtime_once, xhvp_runtime_players_init, NULL);
  return &g_runtime;
}

XhvpPlayer *xhvp_player_lookup(XhvpPlayerId id) {
  XhvpRuntime *rt = xhvp_runtime();
  g_mutex_lock(&rt->players_mu);
  for (int i = 0; i < XHVP_MAX_PLAYERS; i++) {
    if (rt->players[i].in_use && rt->players[i].id == id) {
      XhvpPlayer *p = &rt->players[i];
      g_mutex_unlock(&rt->players_mu);
      return p;
    }
  }
  g_mutex_unlock(&rt->players_mu);
  return NULL;
}

typedef struct {
  GSourceFunc func;
  gpointer data;
  GMutex mu;
  GCond cond;
  gboolean done;
} XhvpInvokeSync;

static gboolean xhvp_invoke_sync_idle(gpointer user_data) {
  XhvpInvokeSync *inv = user_data;
  if (inv->func) {
    inv->func(inv->data);
  }
  g_mutex_lock(&inv->mu);
  inv->done = TRUE;
  g_cond_signal(&inv->cond);
  g_mutex_unlock(&inv->mu);
  return G_SOURCE_REMOVE;
}

void xhvp_runtime_invoke_sync(GSourceFunc func, gpointer data) {
  XhvpRuntime *rt = xhvp_runtime();
  if (!rt->initialized || rt->ctx == NULL) {
    if (func) {
      func(data);
    }
    return;
  }

  if (g_main_context_is_owner(rt->ctx)) {
    if (func) {
      func(data);
    }
    return;
  }

  XhvpInvokeSync inv = {
      .func = func,
      .data = data,
      .done = FALSE,
  };
  g_mutex_init(&inv.mu);
  g_cond_init(&inv.cond);

  GSource *source = g_idle_source_new();
  g_source_set_callback(source, xhvp_invoke_sync_idle, &inv, NULL);
  g_source_attach(source, rt->ctx);
  g_source_unref(source);

  g_mutex_lock(&inv.mu);
  while (!inv.done) {
    g_cond_wait(&inv.cond, &inv.mu);
  }
  g_mutex_unlock(&inv.mu);

  g_mutex_clear(&inv.mu);
  g_cond_clear(&inv.cond);
}

void xhvp_runtime_invoke_async(GSourceFunc func, gpointer data) {
  XhvpRuntime *rt = xhvp_runtime();
  if (!rt->initialized || rt->ctx == NULL) {
    if (func) {
      func(data);
    }
    return;
  }
  GSource *source = g_idle_source_new();
  g_source_set_callback(source, func, data, NULL);
  g_source_attach(source, rt->ctx);
  g_source_unref(source);
}

static gpointer xhvp_runtime_thread_main(gpointer data) {
  XhvpRuntime *rt = data;
  g_main_context_push_thread_default(rt->ctx);
  g_main_loop_run(rt->loop);
  g_main_context_pop_thread_default(rt->ctx);
  return NULL;
}

static void xhvp_log_init_timing(const char *phase, gint64 start_us) {
  const gint64 ms = (g_get_monotonic_time() - start_us) / 1000;
  XHVP_RUNTIME_LOGI("[xhvp-init-timing] %s=%" G_GINT64_FORMAT "ms", phase, ms);
}

/** Performs env/gst_init/thread start. Caller must own the start gate. */
static int32_t xhvp_runtime_start_unlocked(void) {
  XhvpRuntime *rt = xhvp_runtime();
  if (rt->initialized) {
    return XHVP_ERR_OK;
  }

  const gint64 total_us = g_get_monotonic_time();
  gint64 phase_us;

  phase_us = g_get_monotonic_time();
#if defined(__APPLE__) && TARGET_OS_IPHONE
  xhvp_setup_ios_env();
#elif defined(__APPLE__)
  xhvp_setup_macos_env();
#elif defined(_WIN32)
  xhvp_setup_windows_env();
#endif
  xhvp_log_init_timing("native_env_setup", phase_us);

  phase_us = g_get_monotonic_time();
  gst_init(NULL, NULL);
  xhvp_log_init_timing("native_gst_init", phase_us);

#if defined(__APPLE__) && TARGET_OS_IPHONE
  phase_us = g_get_monotonic_time();
  xhvp_register_ios_static_plugins();
  xhvp_register_ios_tls_backend();
  xhvp_log_init_timing("native_ios_plugins", phase_us);
#endif

  phase_us = g_get_monotonic_time();
  rt->ctx = g_main_context_new();
  rt->loop = g_main_loop_new(rt->ctx, FALSE);
  rt->thread = g_thread_new("xhvp-gst", xhvp_runtime_thread_main, rt);
  rt->initialized = true;
  xhvp_log_init_timing("native_thread_start", phase_us);
  xhvp_log_init_timing("native_runtime_start", total_us);
  return XHVP_ERR_OK;
}

int32_t xhvp_runtime_start(void) {
  XhvpRuntime *rt = xhvp_runtime();
  xhvp_start_gate_ensure();

  g_mutex_lock(&g_start_mu);
  while (g_start_in_progress) {
    g_cond_wait(&g_start_cond, &g_start_mu);
  }
  if (rt->initialized) {
    g_mutex_unlock(&g_start_mu);
    return XHVP_ERR_OK;
  }
  g_start_in_progress = TRUE;
  g_mutex_unlock(&g_start_mu);

  const int32_t code = xhvp_runtime_start_unlocked();

  g_mutex_lock(&g_start_mu);
  g_start_result = code;
  g_start_in_progress = FALSE;
  g_cond_broadcast(&g_start_cond);
  g_mutex_unlock(&g_start_mu);
  return code;
}

typedef struct {
  XhvpInitDoneFn cb;
  void *ctx;
} XhvpInitAsyncJob;

static gpointer xhvp_init_async_thread(gpointer data) {
  XhvpInitAsyncJob *job = data;
  const int32_t code = xhvp_init();
  if (job->cb) {
    job->cb(job->ctx, code);
  }
  g_free(job);
  return NULL;
}

void xhvp_init_async(XhvpInitDoneFn cb, void *ctx) {
  XhvpRuntime *rt = xhvp_runtime();
  xhvp_start_gate_ensure();

  g_mutex_lock(&g_start_mu);
  if (rt->initialized) {
    const int32_t code = g_start_result;
    g_mutex_unlock(&g_start_mu);
    if (cb) {
      cb(ctx, code);
    }
    return;
  }
  if (g_start_in_progress) {
    /* Wait on a helper thread so the caller's isolate is not blocked. */
    g_mutex_unlock(&g_start_mu);
    XhvpInitAsyncJob *job = g_new0(XhvpInitAsyncJob, 1);
    job->cb = cb;
    job->ctx = ctx;
    g_thread_unref(g_thread_new("xhvp-init-wait", xhvp_init_async_thread, job));
    return;
  }
  g_mutex_unlock(&g_start_mu);

  XhvpInitAsyncJob *job = g_new0(XhvpInitAsyncJob, 1);
  job->cb = cb;
  job->ctx = ctx;
  g_thread_unref(g_thread_new("xhvp-init", xhvp_init_async_thread, job));
}

void xhvp_runtime_stop(void) {
  XhvpRuntime *rt = xhvp_runtime();
  xhvp_start_gate_ensure();

  g_mutex_lock(&g_start_mu);
  while (g_start_in_progress) {
    g_cond_wait(&g_start_cond, &g_start_mu);
  }
  if (!rt->initialized) {
    g_mutex_unlock(&g_start_mu);
    return;
  }
  g_mutex_unlock(&g_start_mu);

  for (int i = 0; i < XHVP_MAX_PLAYERS; i++) {
    if (rt->players[i].in_use) {
      xhvp_pipeline_destroy(&rt->players[i]);
      rt->players[i].in_use = false;
    }
  }

  if (rt->loop) {
    g_main_loop_quit(rt->loop);
  }
  if (rt->thread) {
    g_thread_join(rt->thread);
    rt->thread = NULL;
  }
  if (rt->loop) {
    g_main_loop_unref(rt->loop);
    rt->loop = NULL;
  }
  if (rt->ctx) {
    g_main_context_unref(rt->ctx);
    rt->ctx = NULL;
  }
  rt->initialized = false;
}
