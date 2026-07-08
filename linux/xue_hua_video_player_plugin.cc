#include "include/xue_hua_video_player/xue_hua_video_player_plugin.h"

#include <flutter_linux/flutter_linux.h>

#include <cstdint>
#include <map>
#include <memory>
#include <mutex>

#include "xue_hua_video_texture.h"

#include <flutter_linux/flutter_linux.h>

#define XUE_HUA_VIDEO_PLAYER_TYPE_PLUGIN (xue_hua_video_player_plugin_get_type())
G_DECLARE_FINAL_TYPE(XueHuaVideoPlayerPlugin, xue_hua_video_player_plugin,
                     XUE_HUA, VIDEO_PLAYER_PLUGIN, GObject)

struct _XueHuaVideoPlayerPlugin {
  GObject parent_instance;
  FlMethodChannel* texture_channel;
  FlTextureRegistrar* texture_registrar;
  std::map<int64_t, XueHuaVideoTexture*>* textures;
  std::mutex* lock;
};

G_DEFINE_TYPE(XueHuaVideoPlayerPlugin, xue_hua_video_player_plugin,
              g_object_get_type())

namespace {

constexpr char kTextureChannelName[] = "xue_hua_video_player/texture";

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

int64_t CreateTexture(XueHuaVideoPlayerPlugin* self, int64_t player_id) {
  if (!self->texture_registrar || player_id == 0) {
    return -1;
  }
  std::lock_guard<std::mutex> guard(*self->lock);
  auto it = self->textures->find(player_id);
  if (it != self->textures->end()) {
    return fl_texture_get_id(FL_TEXTURE(it->second));
  }
  XueHuaVideoTexture* texture =
      xue_hua_video_texture_new(player_id, self->texture_registrar);
  if (!fl_texture_registrar_register_texture(self->texture_registrar,
                                             FL_TEXTURE(texture))) {
    xue_hua_video_texture_dispose_instance(texture, self->texture_registrar);
    return -1;
  }
  (*self->textures)[player_id] = texture;
  return fl_texture_get_id(FL_TEXTURE(texture));
}

void DisposeTexture(XueHuaVideoPlayerPlugin* self, int64_t player_id) {
  if (!self->texture_registrar) {
    return;
  }
  std::lock_guard<std::mutex> guard(*self->lock);
  auto it = self->textures->find(player_id);
  if (it == self->textures->end()) {
    return;
  }
  xue_hua_video_texture_dispose_instance(it->second, self->texture_registrar);
  self->textures->erase(it);
}

void DisposeAllTextures(XueHuaVideoPlayerPlugin* self) {
  if (!self->texture_registrar || !self->textures) {
    return;
  }
  std::lock_guard<std::mutex> guard(*self->lock);
  for (auto& entry : *self->textures) {
    xue_hua_video_texture_dispose_instance(entry.second,
                                           self->texture_registrar);
  }
  self->textures->clear();
}

static void texture_method_call_cb(FlMethodChannel* channel,
                                   FlMethodCall* method_call,
                                   gpointer user_data) {
  auto* self = XUE_HUA_VIDEO_PLAYER_PLUGIN(user_data);
  g_autoptr(FlMethodResponse) response = nullptr;
  const gchar* method = fl_method_call_get_name(method_call);
  FlValue* args = fl_method_call_get_args(method_call);
  const int64_t player_id = PlayerIdFromValue(args);

  if (g_strcmp0(method, "createTexture") == 0) {
    const int64_t texture_id = CreateTexture(self, player_id);
    if (texture_id < 0) {
      response = FL_METHOD_RESPONSE(fl_method_error_response_new(
          "create_failed", "Failed to create texture", nullptr));
    } else {
      response = FL_METHOD_RESPONSE(
          fl_method_success_response_new(fl_value_new_int(texture_id)));
    }
  } else if (g_strcmp0(method, "disposeTexture") == 0) {
    DisposeTexture(self, player_id);
    response = FL_METHOD_RESPONSE(fl_method_success_response_new(nullptr));
  } else {
    response = FL_METHOD_RESPONSE(fl_method_not_implemented_response_new());
  }

  g_autoptr(GError) error = nullptr;
  if (!fl_method_call_respond(method_call, response, &error)) {
    g_warning("Failed to send texture method response: %s", error->message);
  }
}

}  // namespace

static void xue_hua_video_player_plugin_dispose(GObject* object) {
  auto* self = XUE_HUA_VIDEO_PLAYER_PLUGIN(object);
  g_clear_object(&self->texture_channel);
  DisposeAllTextures(self);
  delete self->textures;
  delete self->lock;
  self->textures = nullptr;
  self->lock = nullptr;
  G_OBJECT_CLASS(xue_hua_video_player_plugin_parent_class)->dispose(object);
}

static void xue_hua_video_player_plugin_class_init(
    XueHuaVideoPlayerPluginClass* klass) {
  G_OBJECT_CLASS(klass)->dispose = xue_hua_video_player_plugin_dispose;
}

static void xue_hua_video_player_plugin_init(XueHuaVideoPlayerPlugin* self) {
  self->texture_channel = nullptr;
  self->texture_registrar = nullptr;
  self->textures = new std::map<int64_t, XueHuaVideoTexture*>();
  self->lock = new std::mutex();
}

void xue_hua_video_player_plugin_register_with_registrar(
    FlPluginRegistrar* registrar) {
  auto* plugin = XUE_HUA_VIDEO_PLAYER_PLUGIN(
      g_object_new(xue_hua_video_player_plugin_get_type(), nullptr));

  plugin->texture_registrar =
      fl_plugin_registrar_get_texture_registrar(registrar);

  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  plugin->texture_channel = fl_method_channel_new(
      fl_plugin_registrar_get_messenger(registrar), kTextureChannelName,
      FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(plugin->texture_channel,
                                          texture_method_call_cb, g_object_ref(plugin),
                                          g_object_unref);

  g_object_unref(plugin);
}
