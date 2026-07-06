#include "include/xue_hua_video_player/xue_hua_video_player_plugin.h"

#include <flutter_linux/flutter_linux.h>
#include <gtk/gtk.h>

#include <cstdint>
#include <map>
#include <memory>

#define XUE_HUA_VIDEO_PLAYER_TYPE_PLUGIN (xue_hua_video_player_plugin_get_type())
G_DECLARE_FINAL_TYPE(XueHuaVideoPlayerPlugin, xue_hua_video_player_plugin,
                     XUE_HUA, VIDEO_PLAYER_PLUGIN, GObject)

struct _XueHuaVideoPlayerPlugin {
  GObject parent_instance;
  FlMethodChannel* channel;
  GtkWidget* flutter_view;
  std::map<int64_t, GtkWidget*>* overlays;
};

G_DEFINE_TYPE(XueHuaVideoPlayerPlugin, xue_hua_video_player_plugin,
              g_object_get_type())

extern "C" void player_set_video_overlay_window(int64_t player_id,
                                                int64_t window_handle);

namespace {

constexpr char kChannelName[] = "xue_hua_video_player/desktop_overlay";

int64_t PlayerIdFromValue(FlValue* value) {
  if (!value || fl_value_get_type(value) != FL_VALUE_TYPE_MAP) {
    return 0;
  }
  FlValue* id = fl_value_lookup_string(value, "playerId");
  if (!id || fl_value_get_type(id) != FL_VALUE_TYPE_INT) {
    return 0;
  }
  return fl_value_get_int(id);
}

double DoubleFromValue(FlValue* value) {
  if (!value) {
    return 0.0;
  }
  if (fl_value_get_type(value) == FL_VALUE_TYPE_FLOAT) {
    return fl_value_get_float(value);
  }
  if (fl_value_get_type(value) == FL_VALUE_TYPE_INT) {
    return static_cast<double>(fl_value_get_int(value));
  }
  return 0.0;
}

double DoubleFromMap(FlValue* map, const char* key) {
  if (!map || fl_value_get_type(map) != FL_VALUE_TYPE_MAP) {
    return 0.0;
  }
  return DoubleFromValue(fl_value_lookup_string(map, key));
}

void BindGdkWindow(int64_t player_id, GtkWidget* widget) {
  GdkWindow* window = gtk_widget_get_window(widget);
  if (!window) {
    return;
  }
  player_set_video_overlay_window(
      player_id,
      static_cast<int64_t>(reinterpret_cast<uintptr_t>(window)));
}

void OnOverlayRealize(GtkWidget* widget, gpointer user_data) {
  auto* player_id = static_cast<int64_t*>(user_data);
  if (player_id) {
    BindGdkWindow(*player_id, widget);
  }
}

GtkWidget* CreateOverlayPopup(GtkWidget* flutter_view) {
  GtkWidget* toplevel = gtk_widget_get_toplevel(flutter_view);
  GtkWidget* popup = gtk_window_new(GTK_WINDOW_POPUP);
  gtk_window_set_transient_for(GTK_WINDOW(popup), GTK_WINDOW(toplevel));
  gtk_window_set_decorated(GTK_WINDOW(popup), FALSE);
  gtk_window_set_skip_taskbar_hint(GTK_WINDOW(popup), TRUE);
  gtk_window_set_skip_pager_hint(GTK_WINDOW(popup), TRUE);

  GtkWidget* area = gtk_drawing_area_new();
  gtk_widget_set_hexpand(area, TRUE);
  gtk_widget_set_vexpand(area, TRUE);
  gtk_container_add(GTK_CONTAINER(popup), area);
  gtk_widget_show_all(popup);
  return popup;
}

void AttachOverlay(XueHuaVideoPlayerPlugin* self, int64_t player_id) {
  if (!self->flutter_view || player_id == 0 ||
      self->overlays->count(player_id) != 0) {
    return;
  }
  GtkWidget* popup = CreateOverlayPopup(self->flutter_view);
  GtkWidget* area = gtk_bin_get_child(GTK_BIN(popup));
  auto* id = new int64_t(player_id);
  g_signal_connect(G_OBJECT(area), "realize", G_CALLBACK(OnOverlayRealize), id);
  g_signal_connect(G_OBJECT(popup), "destroy", G_CALLBACK(+[](GtkWidget*, gpointer data) {
                     delete static_cast<int64_t*>(data);
                   }),
                   id);
  if (gtk_widget_get_realized(area)) {
    BindGdkWindow(player_id, area);
  }
  (*self->overlays)[player_id] = popup;
}

void DetachOverlay(XueHuaVideoPlayerPlugin* self, int64_t player_id) {
  auto it = self->overlays->find(player_id);
  if (it == self->overlays->end()) {
    return;
  }
  player_set_video_overlay_window(player_id, 0);
  gtk_widget_destroy(it->second);
  self->overlays->erase(it);
}

void SetOverlayBounds(XueHuaVideoPlayerPlugin* self,
                      int64_t player_id,
                      double x,
                      double y,
                      double width,
                      double height) {
  auto it = self->overlays->find(player_id);
  if (it == self->overlays->end()) {
    return;
  }
  const int w = static_cast<int>(width);
  const int h = static_cast<int>(height);
  gtk_window_move(GTK_WINDOW(it->second), static_cast<int>(x),
                  static_cast<int>(y));
  gtk_window_resize(GTK_WINDOW(it->second), w > 0 ? w : 1, h > 0 ? h : 1);
  GtkWidget* area = gtk_bin_get_child(GTK_BIN(it->second));
  if (area && gtk_widget_get_realized(area)) {
    BindGdkWindow(player_id, area);
  }
}

static void method_call_cb(FlMethodChannel* channel,
                           FlMethodCall* method_call,
                           gpointer user_data) {
  auto* self = XUE_HUA_VIDEO_PLAYER_PLUGIN(user_data);
  g_autoptr(FlMethodResponse) response = nullptr;
  const gchar* method = fl_method_call_get_name(method_call);
  FlValue* args = fl_method_call_get_args(method_call);
  const int64_t player_id = PlayerIdFromValue(args);

  if (g_strcmp0(method, "attach") == 0) {
    AttachOverlay(self, player_id);
    response = FL_METHOD_RESPONSE(fl_method_success_response_new(nullptr));
  } else if (g_strcmp0(method, "detach") == 0) {
    DetachOverlay(self, player_id);
    response = FL_METHOD_RESPONSE(fl_method_success_response_new(nullptr));
  } else if (g_strcmp0(method, "setBounds") == 0) {
    SetOverlayBounds(self, player_id, DoubleFromMap(args, "x"),
                     DoubleFromMap(args, "y"), DoubleFromMap(args, "width"),
                     DoubleFromMap(args, "height"));
    response = FL_METHOD_RESPONSE(fl_method_success_response_new(nullptr));
  } else {
    response = FL_METHOD_RESPONSE(fl_method_not_implemented_response_new());
  }

  g_autoptr(GError) error = nullptr;
  if (!fl_method_call_respond(method_call, response, &error)) {
    g_warning("Failed to send method response: %s", error->message);
  }
}

}  // namespace

static void xue_hua_video_player_plugin_dispose(GObject* object) {
  auto* self = XUE_HUA_VIDEO_PLAYER_PLUGIN(object);
  g_clear_object(&self->channel);
  if (self->overlays) {
    for (auto& entry : *self->overlays) {
      player_set_video_overlay_window(entry.first, 0);
      gtk_widget_destroy(entry.second);
    }
    delete self->overlays;
    self->overlays = nullptr;
  }
  G_OBJECT_CLASS(xue_hua_video_player_plugin_parent_class)->dispose(object);
}

static void xue_hua_video_player_plugin_class_init(
    XueHuaVideoPlayerPluginClass* klass) {
  G_OBJECT_CLASS(klass)->dispose = xue_hua_video_player_plugin_dispose;
}

static void xue_hua_video_player_plugin_init(XueHuaVideoPlayerPlugin* self) {
  self->channel = nullptr;
  self->flutter_view = nullptr;
  self->overlays = new std::map<int64_t, GtkWidget*>();
}

void xue_hua_video_player_plugin_register_with_registrar(
    FlPluginRegistrar* registrar) {
  auto* plugin = XUE_HUA_VIDEO_PLAYER_PLUGIN(
      g_object_new(xue_hua_video_player_plugin_get_type(), nullptr));

  plugin->flutter_view = GTK_WIDGET(fl_plugin_registrar_get_view(registrar));

  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  plugin->channel = fl_method_channel_new(
      fl_plugin_registrar_get_messenger(registrar), kChannelName,
      FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(plugin->channel, method_call_cb,
                                           g_object_ref(plugin), g_object_unref);

  g_object_unref(plugin);
}
