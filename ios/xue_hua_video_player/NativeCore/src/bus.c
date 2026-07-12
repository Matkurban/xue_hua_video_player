#include "xhvp_internal.h"

#include <stdio.h>
#include <string.h>

static int32_t map_gst_state(XhvpPlayer *p, GstState state) {
  switch (state) {
  case GST_STATE_NULL:
    return XHVP_STATE_STOPPED;
  case GST_STATE_READY:
    /* Intermediate READY during play/buffer must not clobber playing/buffering. */
    if (p->player_state == XHVP_STATE_PLAYING ||
        p->player_state == XHVP_STATE_BUFFERING ||
        p->player_state == XHVP_STATE_PAUSED) {
      return p->player_state;
    }
    return XHVP_STATE_READY;
  case GST_STATE_PAUSED:
    /* playbin pauses itself while download-buffering; keep BUFFERING visible. */
    if (p->buffering_percent < 100 && p->desired_playing) {
      return XHVP_STATE_BUFFERING;
    }
    if (p->player_state == XHVP_STATE_BUFFERING) {
      return XHVP_STATE_BUFFERING;
    }
    return XHVP_STATE_PAUSED;
  case GST_STATE_PLAYING:
    if (p->buffering_percent < 100 && p->desired_playing) {
      return XHVP_STATE_BUFFERING;
    }
    return XHVP_STATE_PLAYING;
  default:
    return XHVP_STATE_IDLE;
  }
}

static gboolean xhvp_position_tick(gpointer user_data) {
  XhvpPlayer *p = user_data;
  if (!p->pipeline) {
    return G_SOURCE_CONTINUE;
  }

  gint64 pos = GST_CLOCK_TIME_NONE;
  if (gst_element_query_position(p->pipeline, GST_FORMAT_TIME, &pos) &&
      GST_CLOCK_TIME_IS_VALID(pos)) {
    int64_t ms = (int64_t)(pos / GST_MSECOND);
    if (ms != p->position_ms) {
      p->position_ms = ms;
      xhvp_player_emit(p, XHVP_EVENT_POSITION_CHANGED, "");
    }
  }

  gint64 dur = GST_CLOCK_TIME_NONE;
  if (gst_element_query_duration(p->pipeline, GST_FORMAT_TIME, &dur) &&
      GST_CLOCK_TIME_IS_VALID(dur)) {
    int64_t ms = (int64_t)(dur / GST_MSECOND);
    if (ms != p->duration_ms) {
      p->duration_ms = ms;
      xhvp_player_emit(p, XHVP_EVENT_DURATION_CHANGED, "");
    }
  }
  return G_SOURCE_CONTINUE;
}

static gboolean xhvp_bus_watch_dispatch(gpointer user_data) {
  /* Unused: real handler is xhvp_bus_on_message via gst_bus_create_watch. */
  (void)user_data;
  return G_SOURCE_CONTINUE;
}

static gboolean xhvp_bus_on_message(GstBus *bus, GstMessage *msg,
                                    gpointer user_data) {
  (void)bus;
  XhvpPlayer *p = user_data;

  switch (GST_MESSAGE_TYPE(msg)) {
  case GST_MESSAGE_ERROR: {
    GError *err = NULL;
    gchar *dbg = NULL;
    gst_message_parse_error(msg, &err, &dbg);
    char buf[512];
    snprintf(buf, sizeof(buf), "%s", err ? err->message : "unknown error");
    if (err) {
      g_error_free(err);
    }
    g_free(dbg);
    /* Decodebin/MediaCodec autoplug often emits ERROR from child elements
     * while playbin recovers (e.g. HEVC probe → AVC). Only pipeline-level
     * errors are fatal to the Dart session. */
    if (!p->pipeline || GST_MESSAGE_SRC(msg) != GST_OBJECT(p->pipeline)) {
      strncpy(p->event_message, buf, sizeof(p->event_message) - 1);
      p->event_message[sizeof(p->event_message) - 1] = '\0';
      break;
    }
    xhvp_player_set_state(p, XHVP_STATE_ERROR);
    xhvp_player_emit(p, XHVP_EVENT_ERROR, buf);
    break;
  }
  case GST_MESSAGE_EOS:
    p->at_eos = true;
    if (p->looping) {
      xhvp_pipeline_seek(p, 0);
      xhvp_pipeline_play(p);
    } else {
      xhvp_player_set_state(p, XHVP_STATE_COMPLETED);
      xhvp_player_emit(p, XHVP_EVENT_EOS, "");
    }
    break;
  case GST_MESSAGE_STATE_CHANGED: {
    if (GST_MESSAGE_SRC(msg) != GST_OBJECT(p->pipeline)) {
      break;
    }
    GstState old_s, new_s, pending;
    gst_message_parse_state_changed(msg, &old_s, &new_s, &pending);
    (void)old_s;
    (void)pending;
    if (new_s == GST_STATE_PAUSED || new_s == GST_STATE_PLAYING) {
      xhvp_pipeline_refresh_tracks(p);
      xhvp_pipeline_update_seekable(p);
    }
    xhvp_player_set_state(p, map_gst_state(p, new_s));
    break;
  }
  case GST_MESSAGE_BUFFERING: {
    gint percent = 0;
    gst_message_parse_buffering(msg, &percent);
    if (percent < 0) {
      percent = 0;
    } else if (percent > 100) {
      percent = 100;
    }
    p->buffering_percent = percent;
    if (percent < 100) {
      /* Pause download while prerolling network buffers. */
      if (p->desired_playing && p->pipeline) {
        gst_element_set_state(p->pipeline, GST_STATE_PAUSED);
      }
      xhvp_player_set_state(p, XHVP_STATE_BUFFERING);
    } else if (p->desired_playing) {
      if (p->pending_rate_seek) {
        p->pending_rate_seek = false;
        (void)xhvp_pipeline_apply_rate(p);
      }
#if defined(__ANDROID__)
      /* No window yet — keep pending; do not fake PLAYING without a surface. */
      if (p->android_window == 0) {
        p->pending_auto_play = true;
        xhvp_player_emit(p, XHVP_EVENT_BUFFERING, "");
        break;
      }
#endif
      if (p->pipeline) {
        gst_element_set_state(p->pipeline, GST_STATE_PLAYING);
      }
      xhvp_player_set_state(p, XHVP_STATE_PLAYING);
    } else {
      if (p->pending_rate_seek) {
        p->pending_rate_seek = false;
        (void)xhvp_pipeline_apply_rate(p);
      }
      xhvp_player_set_state(p, XHVP_STATE_PAUSED);
    }
    xhvp_player_emit(p, XHVP_EVENT_BUFFERING, "");
    break;
  }
  case GST_MESSAGE_DURATION_CHANGED: {
    gint64 dur = GST_CLOCK_TIME_NONE;
    if (gst_element_query_duration(p->pipeline, GST_FORMAT_TIME, &dur) &&
        GST_CLOCK_TIME_IS_VALID(dur)) {
      p->duration_ms = (int64_t)(dur / GST_MSECOND);
      xhvp_player_emit(p, XHVP_EVENT_DURATION_CHANGED, "");
    }
    break;
  }
  case GST_MESSAGE_STREAM_COLLECTION: {
    GstStreamCollection *collection = NULL;
    gst_message_parse_stream_collection(msg, &collection);
    if (collection) {
      if (p->stream_collection) {
        gst_object_unref(p->stream_collection);
      }
      p->stream_collection = collection;
    }
    xhvp_pipeline_refresh_tracks(p);
    xhvp_player_emit(p, XHVP_EVENT_TRACKS_CHANGED, "");
    break;
  }
  case GST_MESSAGE_STREAMS_SELECTED:
    xhvp_pipeline_refresh_tracks(p);
    xhvp_pipeline_apply_streams_selected(p, msg);
    xhvp_player_emit(p, XHVP_EVENT_TRACKS_CHANGED, "");
    break;
  default:
    break;
  }
  return TRUE;
}

void xhvp_bus_attach(XhvpPlayer *p) {
  if (!p->pipeline) {
    return;
  }
  xhvp_bus_detach(p);
  XhvpRuntime *rt = xhvp_runtime();
  GstBus *bus = gst_element_get_bus(p->pipeline);
  GSource *bus_src = gst_bus_create_watch(bus);
  /* gst_bus_create_watch expects GstBusFunc-compatible callback. */
  g_source_set_callback(bus_src, (GSourceFunc)(void *)xhvp_bus_on_message, p,
                        NULL);
  (void)xhvp_bus_watch_dispatch;
  p->bus_watch_id = g_source_attach(bus_src, rt->ctx);
  g_source_unref(bus_src);
  gst_object_unref(bus);

  if (p->position_timer_id == 0) {
    GSource *timer = g_timeout_source_new(200);
    g_source_set_callback(timer, xhvp_position_tick, p, NULL);
    p->position_timer_id = g_source_attach(timer, rt->ctx);
    g_source_unref(timer);
  }
}

static void xhvp_source_remove_on_ctx(GMainContext *ctx, guint *id) {
  if (*id == 0) {
    return;
  }
  if (ctx) {
    GSource *src = g_main_context_find_source_by_id(ctx, *id);
    if (src) {
      g_source_destroy(src);
    }
  }
  *id = 0;
}

void xhvp_bus_detach(XhvpPlayer *p) {
  XhvpRuntime *rt = xhvp_runtime();
  xhvp_source_remove_on_ctx(rt->ctx, &p->bus_watch_id);
  xhvp_source_remove_on_ctx(rt->ctx, &p->position_timer_id);
}
