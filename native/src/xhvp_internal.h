#pragma once

#include "xhvp_player.h"

#include <gst/app/gstappsink.h>
#include <gst/gst.h>
#include <pthread.h>
#include <stdbool.h>
#include <stdint.h>

#if defined(__APPLE__)
#include <TargetConditionals.h>
#endif

#define XHVP_MAX_PLAYERS 32
#define XHVP_MAX_TRACKS 64
#define XHVP_ERR_OK 0
#define XHVP_ERR_FAIL -1
#define XHVP_ERR_BAD_ID -2
#define XHVP_ERR_NOT_READY -3

typedef struct XhvpTrackInfo {
  int32_t id;
  int32_t type;
  char language[32];
  char label[128];
  char stream_id[256];
  bool selected;
} XhvpTrackInfo;

typedef struct XhvpFrameBuffer {
  uint8_t *data;
  uint32_t capacity;
  uint32_t size;
  int32_t width;
  int32_t height;
  int32_t stride;
  bool valid;
} XhvpFrameBuffer;

typedef struct XhvpPlayer {
  XhvpPlayerId id;
  bool in_use;

  GstElement *pipeline;
  GstElement *appsink;
  GstElement *appsrc;
  GstElement *orient_element; /* videoflip or glvideoflip; owned by sink bin */
  guint bus_watch_id;
  guint position_timer_id;

  XhvpEventCallback event_cb;
  void *event_ctx;

  XhvpFrameReadyFn frame_cb;
  void *frame_ctx;

  pthread_mutex_t frame_mu;
  XhvpFrameBuffer frames[2];
  int latest_frame;

  double volume;
  bool muted;
  double speed;
  bool looping;
  bool desired_playing;
  bool at_eos;
  bool is_uri;
  bool seekable;
  bool pending_rate_seek;
  int32_t rotate_degrees;
  int32_t aspect_mode;
  int32_t player_state;

  int64_t duration_ms;
  int64_t position_ms;
  int32_t width;
  int32_t height;
  double fps;
  int32_t par_n;
  int32_t par_d;
  int32_t dar_n;
  int32_t dar_d;
  bool interlaced;
  char color_matrix[64];
  char color_range[64];
  char hdr_format[64];
  /* Durable copy for async NativeCallable.listener (no stack pointers). */
  char event_message[512];

  XhvpTrackInfo tracks[XHVP_MAX_TRACKS];
  int32_t track_count;
  GstStreamCollection *stream_collection;
  int32_t buffering_percent;

  /* Android overlay */
  int64_t android_window; /* ANativeWindow* as intptr; owned when non-zero */
  int32_t android_w;
  int32_t android_h;
  bool android_overlay_bound;
  bool pending_auto_play;
  GstElement *overlay_element; /* non-owning; child of video-sink bin */

  uint8_t *asset_bytes;
  uint32_t asset_len;
  uint32_t asset_offset;
  char asset_temp_path[512];
} XhvpPlayer;

typedef struct XhvpRuntime {
  bool initialized;
  GMainContext *ctx;
  GMainLoop *loop;
  GThread *thread;
  pthread_mutex_t players_mu;
  XhvpPlayer players[XHVP_MAX_PLAYERS];
  int64_t next_id;
} XhvpRuntime;

XhvpRuntime *xhvp_runtime(void);
XhvpPlayer *xhvp_player_lookup(XhvpPlayerId id);
int32_t xhvp_runtime_start(void);
void xhvp_runtime_stop(void);
void xhvp_runtime_invoke_sync(GSourceFunc func, gpointer data);
void xhvp_runtime_invoke_async(GSourceFunc func, gpointer data);

#if defined(__APPLE__)
#if defined(TARGET_OS_IPHONE) && TARGET_OS_IPHONE
void xhvp_setup_ios_env(void);
void xhvp_register_ios_static_plugins(void);
void xhvp_register_ios_tls_backend(void);
#else
void xhvp_setup_macos_env(void);
#endif
#endif

void xhvp_player_emit(XhvpPlayer *p, int32_t kind, const char *message);
void xhvp_player_set_state(XhvpPlayer *p, int32_t state);

int32_t xhvp_pipeline_load_uri(XhvpPlayer *p, const char *uri, bool auto_play);
int32_t xhvp_pipeline_load_asset(XhvpPlayer *p, const uint8_t *bytes,
                                 uint32_t len, bool auto_play);
int32_t xhvp_pipeline_play(XhvpPlayer *p);
int32_t xhvp_pipeline_pause(XhvpPlayer *p);
int32_t xhvp_pipeline_stop(XhvpPlayer *p);
int32_t xhvp_pipeline_seek(XhvpPlayer *p, int64_t position_ms);
int32_t xhvp_pipeline_set_volume(XhvpPlayer *p, double volume);
int32_t xhvp_pipeline_set_mute(XhvpPlayer *p, bool mute);
int32_t xhvp_pipeline_set_speed(XhvpPlayer *p, double speed);
int32_t xhvp_pipeline_apply_rate(XhvpPlayer *p);
void xhvp_pipeline_destroy(XhvpPlayer *p);
void xhvp_pipeline_refresh_tracks(XhvpPlayer *p);
void xhvp_pipeline_apply_streams_selected(XhvpPlayer *p, GstMessage *msg);
void xhvp_pipeline_update_seekable(XhvpPlayer *p);
int32_t xhvp_pipeline_select_track(XhvpPlayer *p, int32_t track_id,
                                   int32_t track_type, bool enable);
int32_t xhvp_pipeline_set_rotation(XhvpPlayer *p, int32_t degrees);
int32_t xhvp_pipeline_set_aspect(XhvpPlayer *p, int32_t mode);

void xhvp_bus_attach(XhvpPlayer *p);
void xhvp_bus_detach(XhvpPlayer *p);

void xhvp_frame_init(XhvpPlayer *p);
void xhvp_frame_clear(XhvpPlayer *p);
GstFlowReturn xhvp_frame_on_new_sample(GstAppSink *sink, gpointer user_data);
bool xhvp_frame_info(XhvpPlayer *p, int32_t *w, int32_t *h, int32_t *stride,
                     uint32_t *bytes);
bool xhvp_frame_copy(XhvpPlayer *p, uint8_t *dst, uint32_t dst_len, int32_t *w,
                     int32_t *h, int32_t *stride);

#if defined(__ANDROID__)
void xhvp_android_apply_overlay(XhvpPlayer *p);
void xhvp_android_clear_overlay(XhvpPlayer *p);
void xhvp_android_release_window(XhvpPlayer *p);
GstElement *xhvp_android_make_video_sink(XhvpPlayer *p);
#else
GstElement *xhvp_desktop_make_video_sink(XhvpPlayer *p);
#endif
