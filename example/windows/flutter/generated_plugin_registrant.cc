//
//  Generated file. Do not edit.
//

// clang-format off

#include "generated_plugin_registrant.h"

#include <screen_brightness_windows/screen_brightness_windows_plugin_c_api.h>
#include <xue_hua_video_player/xue_hua_video_player_plugin_c_api.h>

void RegisterPlugins(flutter::PluginRegistry* registry) {
  ScreenBrightnessWindowsPluginCApiRegisterWithRegistrar(
      registry->GetRegistrarForPlugin("ScreenBrightnessWindowsPluginCApi"));
  XueHuaVideoPlayerPluginCApiRegisterWithRegistrar(
      registry->GetRegistrarForPlugin("XueHuaVideoPlayerPluginCApi"));
}
