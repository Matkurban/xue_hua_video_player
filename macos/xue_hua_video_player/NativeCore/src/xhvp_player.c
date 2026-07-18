#include "xhvp_internal.h"

#include <string.h>

#if defined(__ANDROID__)
#include <android/native_window.h>
#endif

void xhvp_player_emit(XhvpPlayer *p, int32_t kind, const char *message) {
  if (!p || !p->event_cb) {
    return;
  }
  /* Copy before callback: Dart NativeCallable.listener is async; stack
   * buffers (e.g. GST_MESSAGE_ERROR) would be freed before Dart reads them. */
  strncpy(p->event_message, message ? message : "",
          sizeof(p->event_message) - 1);
  p->event_message[sizeof(p->event_message) - 1] = '\0';
  p->event_cb(p->event_ctx, kind, p->position_ms, p->duration_ms, p->width,
              p->height, p->buffering_percent, p->player_state, p->event_message,
              p->fps, p->par_n, p->par_d, p->dar_n, p->dar_d, p->interlaced,
              p->color_matrix, p->color_range, p->hdr_format, p->seekable);
}

void xhvp_player_set_state(XhvpPlayer *p, int32_t state) {
  if (!p || p->player_state == state) {
    return;
  }
  p->player_state = state;
  xhvp_player_emit(p, XHVP_EVENT_STATE_CHANGED, "");
}

const char *xhvp_version(void) { return "1.5.4"; }

int32_t xhvp_init(void) { return xhvp_runtime_start(); }

void xhvp_shutdown(void) { xhvp_runtime_stop(); }

XhvpPlayerId xhvp_player_create(void) {
  if (xhvp_init() != XHVP_ERR_OK) {
    return 0;
  }
  XhvpRuntime *rt = xhvp_runtime();
  g_mutex_lock(&rt->players_mu);
  for (int i = 0; i < XHVP_MAX_PLAYERS; i++) {
    if (!rt->players[i].in_use) {
      XhvpPlayer *p = &rt->players[i];
      GMutex frame_mu = p->frame_mu;
      memset(p, 0, sizeof(*p));
      p->frame_mu = frame_mu;
      p->in_use = true;
      p->id = rt->next_id++;
      p->volume = 1.0;
      p->speed = 1.0;
      p->player_state = XHVP_STATE_IDLE;
      p->par_n = 1;
      p->par_d = 1;
      p->buffering_percent = 100;
      p->asset_temp_path[0] = '\0';
      xhvp_frame_init(p);
      XhvpPlayerId id = p->id;
      g_mutex_unlock(&rt->players_mu);
      return id;
    }
  }
  g_mutex_unlock(&rt->players_mu);
  return 0;
}

typedef struct {
  XhvpPlayerId id;
  int32_t result;
} XhvpOp;

static gboolean xhvp_op_dispose(gpointer data) {
  XhvpOp *op = data;
  XhvpPlayer *p = xhvp_player_lookup(op->id);
  if (!p) {
    return G_SOURCE_REMOVE;
  }
#if defined(__ANDROID__)
  xhvp_android_clear_overlay(p);
#endif
  xhvp_pipeline_destroy(p);
  xhvp_frame_clear(p);
  p->event_cb = NULL;
  p->frame_cb = NULL;
  p->in_use = false;
  return G_SOURCE_REMOVE;
}

void xhvp_player_dispose(XhvpPlayerId id) {
  XhvpPlayer *p = xhvp_player_lookup(id);
  if (!p) {
    return;
  }
  XhvpOp op = {.id = id};
  xhvp_runtime_invoke_sync(xhvp_op_dispose, &op);
}

void xhvp_player_set_event_callback(XhvpPlayerId id, void *ctx,
                                    XhvpEventCallback cb) {
  XhvpPlayer *p = xhvp_player_lookup(id);
  if (!p) {
    return;
  }
  p->event_ctx = ctx;
  p->event_cb = cb;
}

typedef struct {
  XhvpPlayerId id;
  char *uri;
  bool auto_play;
  int32_t result;
} XhvpLoadUriOp;

static gboolean xhvp_op_load_uri(gpointer data) {
  XhvpLoadUriOp *op = data;
  XhvpPlayer *p = xhvp_player_lookup(op->id);
  if (!p) {
    op->result = XHVP_ERR_BAD_ID;
    return G_SOURCE_REMOVE;
  }
  op->result = xhvp_pipeline_load_uri(p, op->uri, op->auto_play);
  return G_SOURCE_REMOVE;
}

int32_t xhvp_player_load_uri(XhvpPlayerId id, const char *uri, bool auto_play) {
  if (!xhvp_player_lookup(id)) {
    return XHVP_ERR_BAD_ID;
  }
  XhvpLoadUriOp op = {
      .id = id,
      .uri = g_strdup(uri ? uri : ""),
      .auto_play = auto_play,
      .result = XHVP_ERR_FAIL,
  };
  xhvp_runtime_invoke_sync(xhvp_op_load_uri, &op);
  g_free(op.uri);
  return op.result;
}

typedef struct {
  XhvpPlayerId id;
  uint8_t *bytes;
  uint32_t len;
  bool auto_play;
  int32_t result;
} XhvpLoadAssetOp;

static gboolean xhvp_op_load_asset(gpointer data) {
  XhvpLoadAssetOp *op = data;
  XhvpPlayer *p = xhvp_player_lookup(op->id);
  if (!p) {
    op->result = XHVP_ERR_BAD_ID;
    return G_SOURCE_REMOVE;
  }
  op->result = xhvp_pipeline_load_asset(p, op->bytes, op->len, op->auto_play);
  return G_SOURCE_REMOVE;
}

int32_t xhvp_player_load_asset(XhvpPlayerId id, const char *asset_key,
                               const char *package, const uint8_t *bytes,
                               uint32_t len, bool auto_play) {
  (void)asset_key;
  (void)package;
  if (!xhvp_player_lookup(id)) {
    return XHVP_ERR_BAD_ID;
  }
  XhvpLoadAssetOp op = {
      .id = id,
      .bytes = (uint8_t *)bytes,
      .len = len,
      .auto_play = auto_play,
      .result = XHVP_ERR_FAIL,
  };
  xhvp_runtime_invoke_sync(xhvp_op_load_asset, &op);
  return op.result;
}

#define XHVP_DEFINE_SIMPLE_OP(name, call)                                      \
  static gboolean xhvp_op_##name(gpointer data) {                              \
    XhvpOp *op = data;                                                         \
    XhvpPlayer *p = xhvp_player_lookup(op->id);                                \
    if (!p) {                                                                  \
      op->result = XHVP_ERR_BAD_ID;                                            \
      return G_SOURCE_REMOVE;                                                  \
    }                                                                          \
    op->result = call;                                                         \
    return G_SOURCE_REMOVE;                                                    \
  }                                                                            \
  int32_t xhvp_player_##name(XhvpPlayerId id) {                                \
    if (!xhvp_player_lookup(id))                                               \
      return XHVP_ERR_BAD_ID;                                                  \
    XhvpOp op = {.id = id, .result = XHVP_ERR_FAIL};                           \
    xhvp_runtime_invoke_sync(xhvp_op_##name, &op);                             \
    return op.result;                                                          \
  }

XHVP_DEFINE_SIMPLE_OP(play, xhvp_pipeline_play(p))
XHVP_DEFINE_SIMPLE_OP(pause, xhvp_pipeline_pause(p))
XHVP_DEFINE_SIMPLE_OP(stop, xhvp_pipeline_stop(p))

typedef struct {
  XhvpPlayerId id;
  int64_t position_ms;
  int32_t result;
} XhvpSeekOp;

static gboolean xhvp_op_seek(gpointer data) {
  XhvpSeekOp *op = data;
  XhvpPlayer *p = xhvp_player_lookup(op->id);
  if (!p) {
    op->result = XHVP_ERR_BAD_ID;
    return G_SOURCE_REMOVE;
  }
  op->result = xhvp_pipeline_seek(p, op->position_ms);
  return G_SOURCE_REMOVE;
}

int32_t xhvp_player_seek(XhvpPlayerId id, int64_t position_ms) {
  if (!xhvp_player_lookup(id)) {
    return XHVP_ERR_BAD_ID;
  }
  XhvpSeekOp op = {
      .id = id, .position_ms = position_ms, .result = XHVP_ERR_FAIL};
  xhvp_runtime_invoke_sync(xhvp_op_seek, &op);
  return op.result;
}

typedef struct {
  XhvpPlayerId id;
  double value;
  bool flag;
  int32_t result;
} XhvpScalarOp;

static gboolean xhvp_op_set_volume(gpointer data) {
  XhvpScalarOp *op = data;
  XhvpPlayer *p = xhvp_player_lookup(op->id);
  if (!p) {
    op->result = XHVP_ERR_BAD_ID;
    return G_SOURCE_REMOVE;
  }
  op->result = xhvp_pipeline_set_volume(p, op->value);
  return G_SOURCE_REMOVE;
}

int32_t xhvp_player_set_volume(XhvpPlayerId id, double volume) {
  if (!xhvp_player_lookup(id)) {
    return XHVP_ERR_BAD_ID;
  }
  XhvpScalarOp op = {.id = id, .value = volume, .result = XHVP_ERR_FAIL};
  xhvp_runtime_invoke_sync(xhvp_op_set_volume, &op);
  return op.result;
}

static gboolean xhvp_op_set_mute(gpointer data) {
  XhvpScalarOp *op = data;
  XhvpPlayer *p = xhvp_player_lookup(op->id);
  if (!p) {
    op->result = XHVP_ERR_BAD_ID;
    return G_SOURCE_REMOVE;
  }
  op->result = xhvp_pipeline_set_mute(p, op->flag);
  return G_SOURCE_REMOVE;
}

int32_t xhvp_player_set_mute(XhvpPlayerId id, bool mute) {
  if (!xhvp_player_lookup(id)) {
    return XHVP_ERR_BAD_ID;
  }
  XhvpScalarOp op = {.id = id, .flag = mute, .result = XHVP_ERR_FAIL};
  xhvp_runtime_invoke_sync(xhvp_op_set_mute, &op);
  return op.result;
}

static gboolean xhvp_op_set_speed(gpointer data) {
  XhvpScalarOp *op = data;
  XhvpPlayer *p = xhvp_player_lookup(op->id);
  if (!p) {
    op->result = XHVP_ERR_BAD_ID;
    return G_SOURCE_REMOVE;
  }
  op->result = xhvp_pipeline_set_speed(p, op->value);
  return G_SOURCE_REMOVE;
}

int32_t xhvp_player_set_speed(XhvpPlayerId id, double speed) {
  if (!xhvp_player_lookup(id)) {
    return XHVP_ERR_BAD_ID;
  }
  XhvpScalarOp op = {.id = id, .value = speed, .result = XHVP_ERR_FAIL};
  xhvp_runtime_invoke_sync(xhvp_op_set_speed, &op);
  return op.result;
}

static gboolean xhvp_op_set_looping(gpointer data) {
  XhvpScalarOp *op = data;
  XhvpPlayer *p = xhvp_player_lookup(op->id);
  if (!p) {
    op->result = XHVP_ERR_BAD_ID;
    return G_SOURCE_REMOVE;
  }
  p->looping = op->flag;
  op->result = XHVP_ERR_OK;
  return G_SOURCE_REMOVE;
}

int32_t xhvp_player_set_looping(XhvpPlayerId id, bool looping) {
  if (!xhvp_player_lookup(id)) {
    return XHVP_ERR_BAD_ID;
  }
  XhvpScalarOp op = {.id = id, .flag = looping, .result = XHVP_ERR_FAIL};
  xhvp_runtime_invoke_sync(xhvp_op_set_looping, &op);
  return op.result;
}

int32_t xhvp_player_get_capabilities(XhvpPlayerId id, bool *seek, bool *tracks,
                                     bool *orientation) {
  XhvpPlayer *p = xhvp_player_lookup(id);
  if (!p) {
    return XHVP_ERR_BAD_ID;
  }
  if (seek) {
    *seek = p->seekable;
  }
  if (tracks) {
    *tracks = p->stream_collection != NULL || p->track_count > 0;
  }
  if (orientation) {
    *orientation = true;
  }
  return XHVP_ERR_OK;
}

int32_t xhvp_player_get_track_count(XhvpPlayerId id) {
  XhvpPlayer *p = xhvp_player_lookup(id);
  if (!p) {
    return 0;
  }
  return p->track_count;
}

int32_t xhvp_player_get_track(XhvpPlayerId id, int32_t index, int32_t *out_id,
                              int32_t *out_type, char *language,
                              uint32_t language_len, char *label,
                              uint32_t label_len, bool *selected) {
  XhvpPlayer *p = xhvp_player_lookup(id);
  if (!p || index < 0 || index >= p->track_count) {
    return XHVP_ERR_FAIL;
  }
  XhvpTrackInfo *t = &p->tracks[index];
  if (out_id) {
    *out_id = t->id;
  }
  if (out_type) {
    *out_type = t->type;
  }
  if (language && language_len > 0) {
    strncpy(language, t->language, language_len - 1);
    language[language_len - 1] = '\0';
  }
  if (label && label_len > 0) {
    strncpy(label, t->label, label_len - 1);
    label[label_len - 1] = '\0';
  }
  if (selected) {
    *selected = t->selected;
  }
  return XHVP_ERR_OK;
}

typedef struct {
  XhvpPlayerId id;
  int32_t track_id;
  int32_t track_type;
  bool enable;
  int32_t result;
} XhvpSelectTrackOp;

static gboolean xhvp_op_select_track(gpointer data) {
  XhvpSelectTrackOp *op = data;
  XhvpPlayer *p = xhvp_player_lookup(op->id);
  if (!p) {
    op->result = XHVP_ERR_BAD_ID;
    return G_SOURCE_REMOVE;
  }
  op->result =
      xhvp_pipeline_select_track(p, op->track_id, op->track_type, op->enable);
  return G_SOURCE_REMOVE;
}

int32_t xhvp_player_select_track(XhvpPlayerId id, int32_t track_id,
                                 int32_t track_type, bool enable) {
  if (!xhvp_player_lookup(id)) {
    return XHVP_ERR_BAD_ID;
  }
  XhvpSelectTrackOp op = {.id = id,
                          .track_id = track_id,
                          .track_type = track_type,
                          .enable = enable,
                          .result = XHVP_ERR_FAIL};
  xhvp_runtime_invoke_sync(xhvp_op_select_track, &op);
  return op.result;
}

typedef struct {
  XhvpPlayerId id;
  int32_t degrees;
  int32_t result;
} XhvpRotationOp;

static gboolean xhvp_op_set_rotation(gpointer data) {
  XhvpRotationOp *op = data;
  XhvpPlayer *p = xhvp_player_lookup(op->id);
  if (!p) {
    op->result = XHVP_ERR_BAD_ID;
    return G_SOURCE_REMOVE;
  }
  op->result = xhvp_pipeline_set_rotation(p, op->degrees);
  return G_SOURCE_REMOVE;
}

int32_t xhvp_player_set_video_rotation(XhvpPlayerId id,
                                       int32_t rotate_degrees) {
  if (!xhvp_player_lookup(id)) {
    return XHVP_ERR_BAD_ID;
  }
  XhvpRotationOp op = {
      .id = id, .degrees = rotate_degrees, .result = XHVP_ERR_FAIL};
  xhvp_runtime_invoke_sync(xhvp_op_set_rotation, &op);
  return op.result;
}

int32_t xhvp_player_set_aspect_ratio_mode(XhvpPlayerId id, int32_t mode) {
  XhvpPlayer *p = xhvp_player_lookup(id);
  if (!p) {
    return XHVP_ERR_BAD_ID;
  }
  return xhvp_pipeline_set_aspect(p, mode);
}

typedef struct {
  XhvpPlayerId id;
  int64_t window;
  int32_t w;
  int32_t h;
} XhvpAndroidSurfaceOp;

static gboolean xhvp_op_android_surface(gpointer data) {
  XhvpAndroidSurfaceOp *op = data;
  XhvpPlayer *p = xhvp_player_lookup(op->id);
  if (!p) {
#if defined(__ANDROID__)
    if (op->window != 0) {
      ANativeWindow_release((ANativeWindow *)(intptr_t)op->window);
    }
#endif
    return G_SOURCE_REMOVE;
  }
#if defined(__ANDROID__)
  if (p->android_window != 0 && p->android_window != op->window) {
    /* Different Surface: unbind overlay and drop previous ANativeWindow ref. */
    xhvp_android_clear_overlay(p);
  }
  p->android_window = op->window;
  p->android_w = op->w;
  p->android_h = op->h;
  xhvp_android_apply_overlay(p);
#else
  (void)op;
#endif
  return G_SOURCE_REMOVE;
}

void xhvp_player_notify_android_surface(XhvpPlayerId id, int64_t native_window,
                                        int32_t width, int32_t height) {
  if (!xhvp_player_lookup(id)) {
#if defined(__ANDROID__)
    if (native_window != 0) {
      ANativeWindow_release((ANativeWindow *)(intptr_t)native_window);
    }
#endif
    return;
  }
  /* Sync: GST must hold the new window before Java returns from setSize/bind,
   * so a late onSurfaceCleanup cannot clear android_window before apply. */
  XhvpAndroidSurfaceOp op = {
      .id = id, .window = native_window, .w = width, .h = height};
  xhvp_runtime_invoke_sync(xhvp_op_android_surface, &op);
}

typedef struct {
  XhvpPlayerId id;
} XhvpAndroidClearOp;

static gboolean xhvp_op_clear_android_surface(gpointer data) {
  XhvpAndroidClearOp *op = data;
  XhvpPlayer *p = xhvp_player_lookup(op->id);
  if (p) {
#if defined(__ANDROID__)
    xhvp_android_clear_overlay(p);
#endif
  }
  return G_SOURCE_REMOVE;
}

void xhvp_player_clear_android_surface(XhvpPlayerId id) {
  if (!xhvp_player_lookup(id)) {
    return;
  }
  /* Sync: must finish set_window_handle(0)+ANativeWindow_release before Java
   * destroys the Surface (setSize / release), or glimagesink aborts on a
   * destroyed mutex in eglCreateWindowSurface. */
  XhvpAndroidClearOp op = {.id = id};
  xhvp_runtime_invoke_sync(xhvp_op_clear_android_surface, &op);
}

void xhvp_texture_register(int64_t player_id, void *ctx,
                           XhvpFrameReadyFn on_frame) {
  XhvpPlayer *p = xhvp_player_lookup(player_id);
  if (!p) {
    return;
  }
  p->frame_ctx = ctx;
  p->frame_cb = on_frame;
}

void xhvp_texture_unregister(int64_t player_id) {
  XhvpPlayer *p = xhvp_player_lookup(player_id);
  if (!p) {
    return;
  }
  p->frame_ctx = NULL;
  p->frame_cb = NULL;
}

bool xhvp_texture_frame_info(int64_t player_id, int32_t *out_width,
                             int32_t *out_height, int32_t *out_stride,
                             uint32_t *out_bytes) {
  XhvpPlayer *p = xhvp_player_lookup(player_id);
  if (!p) {
    return false;
  }
  return xhvp_frame_info(p, out_width, out_height, out_stride, out_bytes);
}

bool xhvp_texture_copy_latest(int64_t player_id, uint8_t *dst, uint32_t dst_len,
                              int32_t *out_width, int32_t *out_height,
                              int32_t *out_stride) {
  XhvpPlayer *p = xhvp_player_lookup(player_id);
  if (!p) {
    return false;
  }
  return xhvp_frame_copy(p, dst, dst_len, out_width, out_height, out_stride);
}

typedef struct {
  XhvpPlayerId id;
  uint8_t **out_bgra;
  uint32_t *out_len;
  int32_t *out_width;
  int32_t *out_height;
  int32_t *out_stride;
  int32_t result;
} XhvpCaptureFrameOp;

static gboolean xhvp_op_capture_frame(gpointer data) {
  XhvpCaptureFrameOp *op = data;
  XhvpPlayer *p = xhvp_player_lookup(op->id);
  if (!p) {
    op->result = XHVP_ERR_BAD_ID;
    return G_SOURCE_REMOVE;
  }
  int32_t w = 0;
  int32_t h = 0;
  int32_t stride = 0;
  uint32_t bytes = 0;
  if (!xhvp_frame_info(p, &w, &h, &stride, &bytes) || bytes == 0) {
    op->result = XHVP_ERR_NOT_READY;
    return G_SOURCE_REMOVE;
  }
  uint8_t *buf = g_malloc(bytes);
  if (!xhvp_frame_copy(p, buf, bytes, &w, &h, &stride)) {
    g_free(buf);
    op->result = XHVP_ERR_FAIL;
    return G_SOURCE_REMOVE;
  }
  *op->out_bgra = buf;
  *op->out_len = bytes;
  if (op->out_width) {
    *op->out_width = w;
  }
  if (op->out_height) {
    *op->out_height = h;
  }
  if (op->out_stride) {
    *op->out_stride = stride;
  }
  op->result = XHVP_ERR_OK;
  return G_SOURCE_REMOVE;
}

int32_t xhvp_player_capture_frame(XhvpPlayerId id, uint8_t **out_bgra,
                                  uint32_t *out_len, int32_t *out_width,
                                  int32_t *out_height, int32_t *out_stride) {
  if (!out_bgra || !out_len) {
    return XHVP_ERR_FAIL;
  }
  *out_bgra = NULL;
  *out_len = 0;
  if (!xhvp_player_lookup(id)) {
    return XHVP_ERR_BAD_ID;
  }
  XhvpCaptureFrameOp op = {.id = id,
                           .out_bgra = out_bgra,
                           .out_len = out_len,
                           .out_width = out_width,
                           .out_height = out_height,
                           .out_stride = out_stride,
                           .result = XHVP_ERR_FAIL};
  xhvp_runtime_invoke_sync(xhvp_op_capture_frame, &op);
  return op.result;
}
