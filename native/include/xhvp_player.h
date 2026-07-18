#pragma once

#include <stdbool.h>
#include <stdint.h>

#ifdef _WIN32
#ifdef XHVP_BUILDING
#define XHVP_EXPORT __declspec(dllexport)
#else
#define XHVP_EXPORT __declspec(dllimport)
#endif
#else
/* visibility + used: keep Dart FFI symbols visible to dlsym on Apple Release. */
#define XHVP_EXPORT                                                            \
  __attribute__((visibility("default"))) __attribute__((used))
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef int64_t XhvpPlayerId;

/** Matches Dart [PlayerEventKind] ordinal order. */
enum XhvpEventKind {
  XHVP_EVENT_DURATION_CHANGED = 0,
  XHVP_EVENT_POSITION_CHANGED = 1,
  XHVP_EVENT_VIDEO_SIZE = 2,
  XHVP_EVENT_STATE_CHANGED = 3,
  XHVP_EVENT_BUFFERING = 4,
  XHVP_EVENT_EOS = 5,
  XHVP_EVENT_ERROR = 6,
  XHVP_EVENT_TRACKS_CHANGED = 7,
  XHVP_EVENT_METADATA_CHANGED = 8,
};

/** Matches Dart [PlayerState] ordinal order. */
enum XhvpPlayerState {
  XHVP_STATE_IDLE = 0,
  XHVP_STATE_READY = 1,
  XHVP_STATE_BUFFERING = 2,
  XHVP_STATE_PLAYING = 3,
  XHVP_STATE_PAUSED = 4,
  XHVP_STATE_STOPPED = 5,
  XHVP_STATE_COMPLETED = 6,
  XHVP_STATE_ERROR = 7,
};

/** Matches Dart [TrackType] ordinal order. */
enum XhvpTrackType {
  XHVP_TRACK_AUDIO = 0,
  XHVP_TRACK_VIDEO = 1,
  XHVP_TRACK_SUBTITLE = 2,
};

/** Matches Dart [AspectRatioMode] ordinal order. */
enum XhvpAspectRatioMode {
  XHVP_ASPECT_FIT = 0,
  XHVP_ASPECT_FILL = 1,
  XHVP_ASPECT_STRETCH = 2,
};

typedef void (*XhvpEventCallback)(void *ctx, int32_t kind, int64_t position_ms,
                                  int64_t duration_ms, int32_t width,
                                  int32_t height, int32_t buffering_percent,
                                  int32_t state, const char *message,
                                  double fps, int32_t par_n, int32_t par_d,
                                  int32_t dar_n, int32_t dar_d, bool interlaced,
                                  const char *color_matrix,
                                  const char *color_range,
                                  const char *hdr_format, bool is_seekable);

typedef void (*XhvpFrameReadyFn)(void *ctx);

/** Called when [xhvp_init_async] finishes; [code] is [XHVP_ERR_*]. */
typedef void (*XhvpInitDoneFn)(void *ctx, int32_t code);

XHVP_EXPORT const char *xhvp_version(void);
XHVP_EXPORT int32_t xhvp_init(void);
/**
 * Starts runtime init on a background thread (does not block the caller).
 * Invokes [cb] once finished. Concurrent [xhvp_init] waits on the same gate.
 * If already initialized, [cb] is invoked immediately on the calling thread.
 */
XHVP_EXPORT void xhvp_init_async(XhvpInitDoneFn cb, void *ctx);
XHVP_EXPORT void xhvp_shutdown(void);

XHVP_EXPORT XhvpPlayerId xhvp_player_create(void);
XHVP_EXPORT void xhvp_player_dispose(XhvpPlayerId id);
XHVP_EXPORT void xhvp_player_set_event_callback(XhvpPlayerId id, void *ctx,
                                                XhvpEventCallback cb);

XHVP_EXPORT int32_t xhvp_player_load_uri(XhvpPlayerId id, const char *uri,
                                         bool auto_play);
XHVP_EXPORT int32_t xhvp_player_load_asset(XhvpPlayerId id,
                                           const char *asset_key,
                                           const char *package,
                                           const uint8_t *bytes, uint32_t len,
                                           bool auto_play);

XHVP_EXPORT int32_t xhvp_player_play(XhvpPlayerId id);
XHVP_EXPORT int32_t xhvp_player_pause(XhvpPlayerId id);
XHVP_EXPORT int32_t xhvp_player_stop(XhvpPlayerId id);
XHVP_EXPORT int32_t xhvp_player_seek(XhvpPlayerId id, int64_t position_ms);
XHVP_EXPORT int32_t xhvp_player_set_volume(XhvpPlayerId id, double volume);
XHVP_EXPORT int32_t xhvp_player_set_mute(XhvpPlayerId id, bool mute);
XHVP_EXPORT int32_t xhvp_player_set_speed(XhvpPlayerId id, double speed);
XHVP_EXPORT int32_t xhvp_player_set_looping(XhvpPlayerId id, bool looping);

XHVP_EXPORT int32_t xhvp_player_get_capabilities(XhvpPlayerId id, bool *seek,
                                                 bool *tracks,
                                                 bool *orientation);
XHVP_EXPORT int32_t xhvp_player_get_track_count(XhvpPlayerId id);
XHVP_EXPORT int32_t xhvp_player_get_track(XhvpPlayerId id, int32_t index,
                                          int32_t *out_id, int32_t *out_type,
                                          char *language, uint32_t language_len,
                                          char *label, uint32_t label_len,
                                          bool *selected);
XHVP_EXPORT int32_t xhvp_player_select_track(XhvpPlayerId id, int32_t track_id,
                                             int32_t track_type, bool enable);
XHVP_EXPORT int32_t xhvp_player_set_video_rotation(XhvpPlayerId id,
                                                   int32_t rotate_degrees);
XHVP_EXPORT int32_t xhvp_player_set_aspect_ratio_mode(XhvpPlayerId id,
                                                      int32_t mode);

/** Android: cache ANativeWindow pointer (as intptr) from JNI thread. */
XHVP_EXPORT void xhvp_player_notify_android_surface(XhvpPlayerId id,
                                                    int64_t native_window,
                                                    int32_t width,
                                                    int32_t height);
XHVP_EXPORT void xhvp_player_clear_android_surface(XhvpPlayerId id);

XHVP_EXPORT void xhvp_texture_register(int64_t player_id, void *ctx,
                                       XhvpFrameReadyFn on_frame);
XHVP_EXPORT void xhvp_texture_unregister(int64_t player_id);
XHVP_EXPORT bool xhvp_texture_frame_info(int64_t player_id, int32_t *out_width,
                                         int32_t *out_height,
                                         int32_t *out_stride,
                                         uint32_t *out_bytes);
XHVP_EXPORT bool xhvp_texture_copy_latest(int64_t player_id, uint8_t *dst,
                                          uint32_t dst_len, int32_t *out_width,
                                          int32_t *out_height,
                                          int32_t *out_stride);

/**
 * One-shot cover frame from a URI (no player slot).
 * position_ms < 0 → auto (5% of duration, or 1s). max_width <= 0 → 320.
 * On success *out_bgra is g_malloc'd BGRA; free with xhvp_thumbnail_free.
 */
XHVP_EXPORT int32_t xhvp_thumbnail_capture(
    const char *uri, int64_t position_ms, int32_t max_width, uint8_t **out_bgra,
    uint32_t *out_len, int32_t *out_width, int32_t *out_height,
    int32_t *out_stride);

/** Copy the latest BGRA frame from an active player into a new buffer. */
XHVP_EXPORT int32_t xhvp_player_capture_frame(
    XhvpPlayerId id, uint8_t **out_bgra, uint32_t *out_len, int32_t *out_width,
    int32_t *out_height, int32_t *out_stride);

XHVP_EXPORT void xhvp_thumbnail_free(uint8_t *data);

/**
 * Touch every Dart-looked-up ABI symbol so Apple Release dead-strip / LTO
 * cannot drop them. Call once from the Flutter plugin register path.
 */
XHVP_EXPORT void xhvp_ffi_retain_symbols(void);

#ifdef __cplusplus
}
#endif
