#ifndef FLUTTER_PLUGIN_XUE_HUA_VIDEO_PLAYER_PLUGIN_H_
#define FLUTTER_PLUGIN_XUE_HUA_VIDEO_PLAYER_PLUGIN_H_

#include <flutter_linux/flutter_linux.h>

G_BEGIN_DECLS

#ifdef FLUTTER_PLUGIN_IMPL
#define FLUTTER_PLUGIN_EXPORT __attribute__((visibility("default")))
#else
#define FLUTTER_PLUGIN_EXPORT
#endif

FLUTTER_PLUGIN_EXPORT
G_DECLARE_FINAL_TYPE(XueHuaVideoPlayerPlugin, xue_hua_video_player_plugin,
                     XUE_HUA, VIDEO_PLAYER_PLUGIN, GObject)

FLUTTER_PLUGIN_EXPORT void xue_hua_video_player_plugin_register_with_registrar(
    FlPluginRegistrar* registrar);

G_END_DECLS

#endif  // FLUTTER_PLUGIN_XUE_HUA_VIDEO_PLAYER_PLUGIN_H_
