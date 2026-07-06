#include <flutter/method_channel.h>
#include <flutter/plugin_registrar_windows.h>
#include <flutter/standard_method_codec.h>

#include <memory>
#include <string>

#include "xue_hua_video_platform_view.h"

namespace {

int64_t Int64FromValue(const flutter::EncodableValue& value) {
  if (const auto* n = std::get_if<int32_t>(&value)) {
    return *n;
  }
  if (const auto* n = std::get_if<int64_t>(&value)) {
    return *n;
  }
  if (const auto* n = std::get_if<double>(&value)) {
    return static_cast<int64_t>(*n);
  }
  return 0;
}

int64_t PlayerIdFromArgs(const flutter::EncodableValue& args) {
  const auto* map = std::get_if<flutter::EncodableMap>(&args);
  if (!map) {
    return 0;
  }
  auto it = map->find(flutter::EncodableValue("playerId"));
  if (it == map->end()) {
    return 0;
  }
  return Int64FromValue(it->second);
}

double DoubleFromMap(const flutter::EncodableMap& map,
                     const std::string& key) {
  auto it = map.find(flutter::EncodableValue(key));
  if (it == map.end()) {
    return 0.0;
  }
  if (const auto* n = std::get_if<double>(&it->second)) {
    return *n;
  }
  if (const auto* n = std::get_if<int32_t>(&it->second)) {
    return static_cast<double>(*n);
  }
  if (const auto* n = std::get_if<int64_t>(&it->second)) {
    return static_cast<double>(*n);
  }
  return 0.0;
}

class XueHuaVideoPlayerPlugin : public flutter::Plugin {
 public:
  XueHuaVideoPlayerPlugin(
      flutter::PluginRegistrarWindows* registrar,
      std::shared_ptr<DesktopVideoOverlay> overlay)
      : overlay_(std::move(overlay)) {
    channel_ = std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
        registrar->messenger(), "xue_hua_video_player/desktop_overlay",
        &flutter::StandardMethodCodec::GetInstance());

    channel_->SetMethodCallHandler(
        [overlay = overlay_](const auto& call, auto result) {
          const std::string& method = call.method_name();
          const auto* args = std::get_if<flutter::EncodableMap>(call.arguments());
          if (!args) {
            result->Error("invalid_args", "Expected map arguments");
            return;
          }
          const int64_t player_id = PlayerIdFromArgs(*args);
          if (method == "attach") {
            overlay->Attach(player_id);
            result->Success();
            return;
          }
          if (method == "detach") {
            overlay->Detach(player_id);
            result->Success();
            return;
          }
          if (method == "setBounds") {
            overlay->SetBounds(
                player_id,
                DoubleFromMap(*args, "x"),
                DoubleFromMap(*args, "y"),
                DoubleFromMap(*args, "width"),
                DoubleFromMap(*args, "height"));
            result->Success();
            return;
          }
          result->NotImplemented();
        });
  }

 private:
  std::shared_ptr<DesktopVideoOverlay> overlay_;
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>> channel_;
};

}  // namespace

void XueHuaVideoPlayerPluginRegisterWithRegistrar(
    FlutterDesktopPluginRegistrarRef registrar) {
  auto* windows_registrar =
      flutter::PluginRegistrarManager::GetInstance()
          ->GetRegistrar<flutter::PluginRegistrarWindows>(registrar);

  HWND parent = nullptr;
  if (auto* view = windows_registrar->GetView()) {
    parent = view->GetNativeWindow();
  }
  auto overlay = std::make_shared<DesktopVideoOverlay>(parent);

  auto plugin =
      std::make_unique<XueHuaVideoPlayerPlugin>(windows_registrar, overlay);
  windows_registrar->AddPlugin(std::move(plugin));
}

extern "C" __declspec(dllexport) void XueHuaVideoPlayerPluginCApiRegisterWithRegistrar(
    FlutterDesktopPluginRegistrarRef registrar) {
  XueHuaVideoPlayerPluginRegisterWithRegistrar(registrar);
}
