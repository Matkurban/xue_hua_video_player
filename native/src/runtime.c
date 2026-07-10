#include "xhvp_internal.h"

#include <stdio.h>
#include <string.h>

#if defined(__APPLE__)
#include <TargetConditionals.h>
#endif

static XhvpRuntime g_runtime;
static pthread_once_t g_runtime_once = PTHREAD_ONCE_INIT;

static void xhvp_runtime_players_init(void) {
  pthread_mutex_init(&g_runtime.players_mu, NULL);
  g_runtime.next_id = 1;
  for (int i = 0; i < XHVP_MAX_PLAYERS; i++) {
    g_runtime.players[i].id = 0;
    g_runtime.players[i].in_use = false;
    pthread_mutex_init(&g_runtime.players[i].frame_mu, NULL);
  }
}

XhvpRuntime *xhvp_runtime(void) {
  pthread_once(&g_runtime_once, xhvp_runtime_players_init);
  return &g_runtime;
}

XhvpPlayer *xhvp_player_lookup(XhvpPlayerId id) {
  XhvpRuntime *rt = xhvp_runtime();
  pthread_mutex_lock(&rt->players_mu);
  for (int i = 0; i < XHVP_MAX_PLAYERS; i++) {
    if (rt->players[i].in_use && rt->players[i].id == id) {
      XhvpPlayer *p = &rt->players[i];
      pthread_mutex_unlock(&rt->players_mu);
      return p;
    }
  }
  pthread_mutex_unlock(&rt->players_mu);
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

int32_t xhvp_runtime_start(void) {
  XhvpRuntime *rt = xhvp_runtime();
  if (rt->initialized) {
    return XHVP_ERR_OK;
  }

#if defined(__APPLE__) && TARGET_OS_IPHONE
  xhvp_setup_ios_env();
#elif defined(__APPLE__)
  xhvp_setup_macos_env();
#endif

  gst_init(NULL, NULL);

#if defined(__APPLE__) && TARGET_OS_IPHONE
  xhvp_register_ios_static_plugins();
  xhvp_register_ios_tls_backend();
#endif

  rt->ctx = g_main_context_new();
  rt->loop = g_main_loop_new(rt->ctx, FALSE);
  rt->thread = g_thread_new("xhvp-gst", xhvp_runtime_thread_main, rt);
  rt->initialized = true;
  return XHVP_ERR_OK;
}

void xhvp_runtime_stop(void) {
  XhvpRuntime *rt = xhvp_runtime();
  if (!rt->initialized) {
    return;
  }

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
