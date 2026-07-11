# xue_hua_video_player

[English](README.md) | 简体中文

一个跨平台的 Flutter **视频播放器**插件：底层通过 **原生 C 核心 + Dart FFI** 驱动
**GStreamer** 解码本地与网络视频，并通过 Flutter 外部 **`Texture`** 与自定义原生桥接渲染
（Android：`glimagesink` + `SurfaceProducer`；Apple/桌面：`appsink` BGRA 帧）。

- 仓库地址：<https://github.com/Matkurban/xue_hua_video_player>
- 作者：Matkurban &lt;3496354336@qq.com&gt;

支持平台：**Android、iOS、macOS、Windows、Linux**。

> 功能范围：仅做视频播放 —— 打开 / 播放 / 暂停 / 停止 / 跳转 / 音量 / 静音 / 倍速 /
> 循环，以及状态 / 进度 / 时长 / 分辨率 / 缓冲 / 播放结束（EOS）/ 错误上报。**不包含**
> 录制、作为服务端推流、字幕轨道选择等功能。

## 目录

- [功能特性](#功能特性)
- [平台支持](#平台支持)
- [安装](#安装)
- [快速上手](#快速上手)
- [在应用中集成（请先阅读）](#在应用中集成请先阅读)
- [权限与各平台配置](#权限与各平台配置)
- [Apple Release / FFI 符号（iOS 与 macOS）](#apple-release--ffi-符号ios-与-macos)
- [API 说明](#api-说明)
- [各平台的 GStreamer 原生环境配置](#各平台的-gstreamer-原生环境配置)
- [架构](#架构)
- [常见问题](#常见问题)
- [维护者](#维护者)
- [许可证](#许可证)

## 功能特性

- 支持本地文件、Flutter 资源（asset）以及网络地址（`http(s)://`、`rtsp://` 等）。
- 播放 / 暂停 / 停止 / 跳转 / 循环。
- 音量、静音、倍速控制。
- 基于细粒度 [`signals`] 的响应式状态：播放状态、进度、时长、视频尺寸、宽高比、
  缓冲百分比、音量、倍速、循环、静音、错误等。
- 开箱即用的 `XueHuaVideoView` 组件，内置可自动隐藏、可主题化的控制条
  （Material / Cupertino / 自适应）。
- 通过 Flutter `Texture` 渲染（Android GL 写入 `SurfaceProducer`；Apple/桌面由 `appsink` 供帧）。

## 平台支持

| 平台 | 最低版本 | 架构 | GStreamer 运行时 |
| --- | --- | --- | --- |
| Android | API 24（7.0） | `arm64-v8a`、`armeabi-v7a`、`x86`、`x86_64` | 构建时自动下载 Android SDK 并 ndk-build |
| iOS | 13.0 | 真机 `arm64`（不支持模拟器） | GStreamer iOS SDK（静态 framework） |
| macOS | 10.13 | x86_64 / arm64 | Homebrew 或 `GStreamer.framework` |
| Windows | 10+ | x86_64 | GStreamer MSVC 运行时 |
| Linux | — | x86_64 | 系统 GStreamer + GTK 3 |

> 不支持 Apple Silicon 的 iOS **模拟器**，因为官方预编译 iOS SDK 不提供 arm64 模拟器切片。
>
> 在 Apple Silicon 的 macOS 上，Homebrew 默认安装的 GStreamer 通常只有 `arm64`
> 切片。插件在 Homebrew 调试模式下默认构建 `arm64`；**Mac App Store / universal 发布**
> 会在 `pod install` 时自动下载官方 universal `GStreamer.framework` 到用户缓存。

## 安装

从 [pub.dev](https://pub.dev/packages/xue_hua_video_player) 添加依赖到应用的
`pubspec.yaml`：

```yaml
dependencies:
  xue_hua_video_player: ^1.5.0
```

然后执行：

```bash
flutter pub get
```

原生播放器核心（`native/`）在应用构建时以 **C** 编译（CMake / CocoaPods / NDK），
**不需要** Rust 工具链。各平台在构建时都需要 GStreamer SDK（部分平台运行时也需要其运行库）。
Android 会在**首次构建时自动下载**官方 GStreamer Android SDK，并通过 ndk-build 生成伞形
`libgstreamer_android.so`（见下文 [Android](#android全部-abi)）。

## 快速上手

```dart
import 'package:flutter/material.dart';
import 'package:xue_hua_video_player/xue_hua_video_player.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  // 应用启动时初始化一次原生库。
  await XueHuaVideoPlayer.initialize();
  runApp(const MyApp());
}

class PlayerPage extends StatefulWidget {
  const PlayerPage({super.key});
  @override
  State<PlayerPage> createState() => _PlayerPageState();
}

class _PlayerPageState extends State<PlayerPage> {
  final controller = XueHuaPlayerController();

  @override
  void initState() {
    super.initState();
    controller.initialize().then((_) {
      controller.open(
        VideoSource.network('https://example.com/video.mp4'),
        autoPlay: true,
      );
    });
  }

  @override
  void dispose() {
    controller.dispose(); // 务必释放，回收原生播放器
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: XueHuaVideoView(controller: controller),
    );
  }
}
```

数据源也可以是本地文件或打包资源：

```dart
await controller.open(const VideoSource.file('/path/to/video.mp4'));
await controller.open(const VideoSource.asset('assets/sample.mp4'));
```

打包资源由 Dart FFI 读入字节后写入原生临时文件加载（非 AppSrc）。

## 在应用中集成（请先阅读）

1. **在创建任何控制器之前，先调用一次 `XueHuaVideoPlayer.initialize()`**（通常放在
   `main()` 里、`WidgetsFlutterBinding.ensureInitialized()` 之后）。该方法幂等，热重启后
   再次调用也安全。
2. **每个视频画面创建并 `initialize()` 一个 `XueHuaPlayerController`。** 控制器持有原生
   播放器，在 `initialize()` 时创建。
3. **画面销毁时务必调用 `dispose()`** —— 它会停止管线、取消事件流并释放原生资源。忘记
   释放会泄漏原生管线。
4. **在 `SignalBuilder`/`Watch` 内读取状态。** 所有状态字段都是 `ReadonlySignal`；在响应式
   builder 之外读取 `.value` 不会在其变化时触发重建。
5. **Android 默认构建全部四种 ABI**（`arm64-v8a`、`armeabi-v7a`、`x86`、`x86_64`）。可按需用
   `abiFilters` 收窄以减小 APK 体积（见下文）。首次 Android 构建会下载 GStreamer Android
   SDK 并运行 ndk-build（需要网络）。
6. **所有平台在构建时都需要 GStreamer SDK**（部分平台运行时也需要其运行库）。见
   [各平台的 GStreamer 原生环境配置](#各平台的-gstreamer-原生环境配置)。

## 权限与各平台配置

播放**网络**视频需要按平台进行配置。本地文件与资源无需额外权限（正常文件访问即可）。

### Android

在应用的 `android/app/src/main/AndroidManifest.xml` 中（`<application>` 之外）添加网络权限：

```xml
<uses-permission android:name="android.permission.INTERNET"/>
```

如需允许明文 `http://`（Android 在 API 28+ 默认禁止明文流量），在 `<application>` 上设置
`usesCleartextTraffic`：

```xml
<application
    android:usesCleartextTraffic="true"
    ...>
```

插件默认构建全部四种 ABI（`arm64-v8a`、`armeabi-v7a`、`x86`、`x86_64`），因此**无需**
配置 `abiFilters`，除非你想缩小 APK。若要收窄，可在 `android/app/build.gradle(.kts)` 中设置：

```kotlin
android {
    defaultConfig {
        ndk {
            // 可选：只保留需要的 ABI（arm64-v8a 覆盖多数现代手机；x86_64 便于模拟器）。
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }
}
```

建议使用按 ABI 拆分的 APK 或 Android App Bundle，让每台设备只下载自己的 ABI —— 每个 ABI
打包的 GStreamer 运行时较大（约 13–18 MB）。

> 插件为完整性也提供了 `x86`（32 位）库，但当前 Flutter 已不再构建 32 位 x86 应用，因此
> 实际上 Flutter 应用真正打包的是 `arm64-v8a`、`armeabi-v7a` 与 `x86_64`。

说明：

- 插件的 `AndroidManifest.xml` 已设置 `android:extractNativeLibs="true"`，会合并进你的应用，
  确保 `libgstreamer_android.so` 被解压到磁盘供动态加载器使用。
- 部分传递依赖需要较新的 `compileSdk`。若 AAR 元数据校验失败，可在所有子工程强制
  `compileSdk = 36`（示例工程在其 `android/build.gradle.kts` 中就是这样做的）。

#### Release 构建（R8 / ProGuard）

视频通过 Flutter **`Texture`**（`SurfaceProducer` 表面）渲染；GStreamer `glimagesink`
经 `VideoOverlay` 绑定。
插件 AAR 已内置 consumer ProGuard 规则（`android/proguard-rules.pro`）。请保留 GStreamer
JNI 辅助类：

```proguard
-keep class org.freedesktop.gstreamer.** { *; }
```

在 `isMinifyEnabled = true` 时确保 `release` 构建类型引用了 `proguard-rules.pro`。

**v1.4.0+** 全平台使用 Flutter 外部 `Texture`（自定义桥接，无 `irondash_texture`）。
Android 仍遵循
[GStreamer Android 视频教程](https://gstreamer.freedesktop.org/documentation/tutorials/android/video.html)。

**v1.0.19+** 采用 GStreamer Android 教程的线程模型：专用 `xhvp-gst` 线程与自有
`GMainContext`，所有管线操作在该线程上执行。

### iOS

- **最低部署版本：iOS 13.0。** 仅支持真机 `arm64`（不支持模拟器）。
- GStreamer 使用自身的 socket + OpenSSL 进行网络通信，**不**走 `NSURLSession`，因此 App
  Transport Security（ATS）**不会**拦截播放，GStreamer 的流也**无需**配置
  `NSAppTransportSecurity`（`http://` 与 `https://` 均可播放）。
- 静态 iOS framework 的插件与 TLS 后端在 C 核心中注册（`native/src/ios_plugins.c`、
  `native/src/ios_tls.c`，由 `native/src/runtime.c` 调用）。为兼容那些不在内置 CA 链中的主机，HTTPS 证书校验被有意
  放宽（`ssl-strict = false`）—— 见[HTTPS 安全提示](#https-安全提示)。

### macOS

Mac App Store 要求开启 **App Sandbox**。运行时 C 核心会为已嵌入的 framework 配置
`GST_PLUGIN_SYSTEM_PATH` 与 `GIO_MODULE_DIR`。

**重要：** Flutter 通过 **Swift Package Manager（SPM）** 集成本插件时（存在
`Package.swift` 时的默认路径），CocoaPods 的 `vendored_frameworks` **不会**执行，
因此 **不会**自动把 GStreamer 拷进 `.app`。缺少嵌入步骤会出现：

```text
dyld: Library not loaded: @rpath/GStreamer.framework/Versions/1.0/lib/GStreamer
```

#### 嵌入 GStreamer（带 Podfile 的 SPM 宿主必做）

在 `macos/Podfile` 中解析插件路径，并在 `post_install` 调用 helper：

```ruby
require 'json'
plugins = JSON.parse(File.read(File.expand_path('../.flutter-plugins-dependencies', __dir__)))
xhvp = plugins.dig('plugins', 'macos')&.find { |p| p['name'] == 'xue_hua_video_player' }
raise 'xue_hua_video_player not found; run flutter pub get first' unless xhvp
require File.expand_path('macos/gstreamer_podfile_helper.rb', xhvp['path'])

# ...

post_install do |installer|
  installer.pods_project.targets.each do |target|
    flutter_additional_macos_build_settings(target)
  end
  install_gstreamer_embed_script!(installer)
end
```

然后执行 `cd macos && pod install`。这会给 Runner 增加 Run Script，把 slim runtime
拷到 `YourApp.app/Contents/Frameworks/GStreamer.framework`。

无 Podfile 的纯 SPM 工程（本仓库 `example`）：保留调用
`macos/scripts/embed_gstreamer_framework.sh` 的 Runner Run Script。

在 `macos/Runner/DebugProfile.entitlements` 和 `Release.entitlements` 中至少加入：

```xml
<key>com.apple.security.app-sandbox</key>
<true/>
<key>com.apple.security.network.client</key>
<true/>
```

完整上架步骤见 [Mac App Store 发布（macOS）](#mac-app-store-发布macos)。

本地调试若暂未安装官方 Framework，可设置 `XUE_HUA_ALLOW_HOMEBREW_GSTREAMER=1` 使用
Homebrew GStreamer（**不可用于上架**）。

### Apple Release / FFI 符号（iOS 与 macOS）

在 Apple 平台上，Dart 通过 `DynamicLibrary.process()` 加载 C 播放器，并用 `dlsym`
解析 `xhvp_*`。Release / Archive 可能剥离这些全局符号，表现为：

```text
Failed to lookup symbol 'xhvp_init': dlsym(RTLD_DEFAULT, xhvp_init): symbol not found
```

插件侧已做：

- ABI 导出带 `__attribute__((used))`，并在插件 `register` 中调用
  `xhvp_ffi_retain_symbols()`（避免 dead-code strip 删掉符号）。
- CocoaPods：通过 `user_target_xcconfig` 向 Runner 注入
  `-force_load libxue_hua_video_player.a` 与 `STRIP_STYLE=non-global`。

宿主仍需保证 **Strip Style = Non-Global Symbols**，否则 Archive 后 `dlsym` 看不到
全局符号名。另见
[Flutter C interop — Stripping symbols](https://docs.flutter.dev/platform-integration/ios/c-interop)。

| 集成方式 | 是否需要手动配置 Strip Style |
| --- | --- |
| **CocoaPods**（多数带 `Podfile` 的 Flutter 应用） | 一般**不需要**。升级本插件后执行 `cd ios && pod install` 和/或 `cd macos && pod install`。可在 Xcode 中确认 Runner → Build Settings → Strip Style 在 Release/Profile 下为 **Non-Global Symbols**。 |
| **Swift Package Manager（SPM）** | **需要**。podspec 不会生效。按下方步骤在 Xcode 中配置。本仓库 `example` 已预置 `STRIP_STYLE = non-global`。 |

#### SPM / 手动 Xcode 步骤

1. 打开 `ios/Runner.xcworkspace` 或 `macos/Runner.xcworkspace`。
2. 选中 Target **Runner** → **Build Settings**。
3. 搜索 **Strip Style**（或 `STRIP_STYLE`）。
4. 将 **Release**（以及用于 TestFlight/分发的 **Profile**）从 **All Symbols** 改为
   **Non-Global Symbols**。
5. Clean 后重新构建：`flutter build ios --release` / `flutter build macos --release`
   （或 Archive）。

也可直接在 `project.pbxproj` 的 Release/Profile 配置中写入：

```
STRIP_STYLE = non-global;
```

#### 如何确认配置生效

```bash
# macOS Release .app
nm -gU YourApp.app/Contents/MacOS/YourApp | grep xhvp_init

# iOS（.app 内的 Runner 二进制）
nm -gU Runner.app/Runner | grep xhvp_init
```

应能看到 `_xhvp_init`。若无输出，说明符号仍被剥离。

### Windows

- 从 <https://gstreamer.freedesktop.org/download/> 安装 GStreamer 的 **MSVC** 包
  （开发文件 + 运行时）。
- 确保运行时 DLL 在运行时可被找到 —— 把 `...\1.0\msvc_x86_64\bin` 加入 `PATH`，或把 DLL
  与你的 `.exe` 放在一起。
- 插件 DLL 位于 `lib\gstreamer-1.0`；打包时在启动阶段把 `GST_PLUGIN_SYSTEM_PATH` 指向打包
  副本。

### Linux

- 无需应用权限。安装系统的 GStreamer 开发/插件包以及 GTK 3（见
  [Linux 原生配置](#linux-1)）。

### HTTPS 安全提示

为了最大化兼容那些证书链不在（精简的）内置信任库中的主机，管线在 HTTP 源上设置了
`ssl-strict = false`，即**跳过服务端证书校验**。这会失去对中间人攻击的防护。如果你需要严格
校验，请打包一个 CA 证书库并配置 GLib 的默认 `GTlsDatabase`（如需将此项做成可配置，欢迎提
issue）。

## API 说明

### `XueHuaVideoPlayer.initialize()`

对原生库 / Rust 桥接进行一次性初始化。使用任何控制器前调用一次。

### `XueHuaPlayerController`

| 方法 | 说明 |
| --- | --- |
| `initialize()` | 创建原生播放器并订阅事件。 |
| `open(VideoSource, {bool autoPlay})` | 加载数据源；可选自动播放。 |
| `play()` / `pause()` / `stop()` | 播放传输控制。 |
| `togglePlayPause()` | 暂停时播放，播放时暂停。 |
| `seek(Duration)` | 跳转到指定位置。 |
| `setVolume(double)` | 音量，范围 `0.0..1.0`。 |
| `setMuted(bool)` / `toggleMuted()` | 静音控制。 |
| `setSpeed(double)` | 倍速。 |
| `setLooping(bool)` | 结束时循环。 |
| `queryPosition()` / `queryDuration()` | 直接向管线查询。 |
| `dispose()` | 拆除播放器并释放全部资源。 |

响应式状态（均为 `ReadonlySignal`，请在 `SignalBuilder` 中读取 `.value`）：
`state`、`position`、`duration`、`videoSize`、`aspectRatio`、`bufferingPercent`、
`volume`、`speed`、`looping`、`muted`、`isPlaying`、`isCompleted`、`error`、
`playerId`、`initialized`。

`PlayerState`：`idle`、`ready`、`buffering`、`playing`、`paused`、`stopped`、
`completed`、`error`。

### `VideoSource`

- `VideoSource.network(String url)`
- `VideoSource.file(String path)`（接受普通路径或 `file://` URI）
- `VideoSource.asset(String assetKey)`

### `XueHuaVideoView`

一个 `StatelessWidget`，为控制器嵌入 `Texture` 视频画面，并默认叠加自适应控制条。

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `controller` | 必填 | 要渲染的 `XueHuaPlayerController`。 |
| `aspectRatioMode` | `AspectRatioMode.fit` | GStreamer sink 缩放（`fit` / `fill` / `stretch`）；亦可通过 `controller.setAspectRatioMode` 设置。 |
| `backgroundColor` | 黑色 | 黑边 / 背景颜色。 |
| `showControls` | `true` | 是否叠加内置控制条。 |
| `controlsStyle` | `adaptive` | `adaptive` / `material` / `cupertino`。 |

### 控制条主题

在 `ThemeData.extensions` 中注册 `VideoControlsTheme` 以自定义控制条，或使用内置的
`VideoControlsTheme.material()` / `VideoControlsTheme.cupertino()` 预设：

```dart
MaterialApp(
  theme: ThemeData(
    extensions: const [/* 你的 */ VideoControlsTheme.material()],
  ),
);
```

## 各平台的 GStreamer 原生环境配置

Rust 核心在构建时链接 GStreamer，所以构建时必须能找到 GStreamer 的**开发**安装，运行时必须
能获得其运行库（内置或系统安装）。

### macOS

**Mac App Store / 沙盒发布**必须使用官方 universal `GStreamer.framework`（x86_64 + arm64），
不能用 Homebrew 路径下的 dylib。

首次 `pod install` 时会**自动下载** runtime + devel 到用户缓存（合计约 **800MB–1GB** 下载，无需 sudo）：

`~/Library/Caches/xue_hua_video_player/gstreamer/1.28.4/`
  - `GStreamer.framework` — 完整 SDK（构建链接用）
  - `GStreamerRuntime.framework` — runtime 快照（嵌入 `.app` 用，消费方无需关心）

最终 `.app` 嵌入经裁剪的 **Slim Runtime**（v1.0.5+，约 **350–450MB** universal，或 **175–280MB** 单架构），
仅保留 playbin3 + HTTPS/HLS/RTSP + applemedia 硬解所需插件。多项目共享同一份下载缓存。

按架构分包（可选）：构建前设置 `XUE_HUA_GSTREAMER_ARCH=arm64` 或 `x86_64`（默认 `universal`），
可分别产出 Apple Silicon / Intel 安装包以减小体积。

可选环境变量：`XUE_HUA_GSTREAMER_ROOT`、`GSTREAMER_FRAMEWORK_SRC`（离线/自定义路径）；
维护者仍可用 `sh tool/setup_gstreamer_macos.sh --system` 安装到 `/Library/Frameworks`。

#### 消费方：构建配置

1. 开启 App Sandbox + `com.apple.security.network.client`（见 [权限](#权限与各平台配置)）。
2. 执行 `flutter pub get`。
3. 按 [macOS](#macos) 将 GStreamer embed helper 写入 `macos/Podfile` 的
   `post_install`（**SPM 下必做**），再 `cd macos && pod install`（首次会按需下载
   GStreamer 缓存）。
4. `flutter build macos --release`，确认产物中存在
   `YourApp.app/Contents/Frameworks/GStreamer.framework`：

```bash
ls YourApp.app/Contents/Frameworks/GStreamer.framework/Versions/1.0/lib/GStreamer
```

若该路径不存在，启动会报
`Library not loaded: @rpath/GStreamer.framework/...`。

C 核心会在 `gst_init()` 前设置 `GST_PLUGIN_SYSTEM_PATH`、`GIO_MODULE_DIR` 以及沙盒可写的
`GST_REGISTRY` 路径（见 `native/src/apple_env.c` 中的 `xhvp_setup_macos_env()`）。

#### 本地 Homebrew 调试（非 MAS）

```bash
export XUE_HUA_ALLOW_HOMEBREW_GSTREAMER=1
brew install pkg-config gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-libav
```

### Mac App Store 发布（macOS）

1. `macos/Runner/*entitlements` 开启沙盒与 `network.client`。
2. `flutter build macos --release` 或 Xcode Archive（首次构建会自动下载 GStreamer 缓存）。
3. 验证：
   - `YourApp.app/Contents/Frameworks/GStreamer.framework` 存在
   - `codesign -vvv --deep --strict YourApp.app` 通过
   - 沙盒下可播放 http/https 视频
4. Validate App → Upload to App Store Connect。

### Linux

```bash
sudo apt install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-libav \
  libgtk-3-dev
```

运行时使用系统 GStreamer 库。因为纹理后端链接了 GTK 3，所以需要 `libgtk-3-dev`。

### Windows

> GStreamer 1.28+ 改为提供单个 **Inno Setup `.exe`** 安装器（旧的 `.msi` 包，包括单独的
> `-devel` 包，已被移除）。一个安装器同时包含运行时与开发文件。

1. 从 <https://gstreamer.freedesktop.org/download/> 下载
   `gstreamer-1.0-msvc-x86_64-<版本>.exe`（MSVC 构建）并运行。选择
   **“Runtime and development headers”**（运行时与开发头文件）安装类型，以便安装
   头文件 / `.lib` / `.pc` 文件。
2. GUI 安装器会设置 `GSTREAMER_1_0_ROOT_MSVC_X86_64`，`windows/CMakeLists.txt` 会用它定位
   头文件/库，并把运行时 DLL 打包到应用旁。（在无界面/静默安装时该变量可能不会被可靠设置，
   必要时请手动设置。）
3. `PATH` 上需要有 `pkg-config`（例如 `choco install pkgconfiglite`）。
4. 插件 DLL 位于 `lib/gstreamer-1.0`；打包时在启动阶段把 `GST_PLUGIN_SYSTEM_PATH` 指向打包
   副本。

### iOS（真机）

面向 arm64 真机 iPhone。预编译 SDK 不提供 arm64 模拟器切片，因此不支持 Apple Silicon 的
iOS 模拟器。

1. 下载并安装 **GStreamer iOS SDK**（`devel`），版本主/次号需与桌面端 GStreamer 匹配：

   ```bash
   curl -fLO https://gstreamer.freedesktop.org/data/pkg/ios/1.28.4/gstreamer-1.0-devel-1.28.4-ios-universal.pkg
   # 用户域安装（无需 sudo）；双击 .pkg 也可以
   installer -pkg gstreamer-1.0-devel-1.28.4-ios-universal.pkg -target CurrentUserHomeDirectory
   ```

   这会把 `GStreamer.framework` 安装到 `~/Library/Developer/GStreamer/iPhone.sdk`
   （可用 `GSTREAMER_ROOT_IOS` 覆盖）。
2. `ios/xue_hua_video_player.podspec` 已为 Rust 交叉编译导出 `system-deps` 覆盖项、
   `PKG_CONFIG_ALLOW_CROSS=1`，并通过 `ios/scripts/ios_rust_link_flags.sh` 注入
   `RUSTFLAGS`（链接 UIKit/OpenGLES/QuartzCore 等，供 Cargokit 编译 staticlib 时使用）；
   Xcode 最终链接使用 `OTHER_LDFLAGS` 中的 `-framework GStreamer` 等参数。
3. 连接真机后构建/运行：`flutter run -d <device>`（或用 `flutter build ios --no-codesign`
   验证构建）。

由于 iOS 的 `GStreamer.framework` 是静态的，其插件不会被自动发现。C 核心会注册播放所需
的插件与 OpenSSL TLS 后端（`native/src/runtime.c` 中的 `xhvp_register_ios_static_plugins()` /
`xhvp_register_ios_tls_backend()`，仅 iOS），并在 `gst_init()` 之前准备运行环境。若需要尚未
注册的元件，请把它加入 `native/src/ios_plugins.c` 的插件列表。

### Android（全部 ABI）

支持 `arm64-v8a`、`armeabi-v7a`、`x86`、`x86_64`。每次 Android 构建时插件会：

1. 下载官方 GStreamer Android universal SDK（若缓存中不存在）
2. 通过 ndk-build 为每个 ABI 生成伞形 `libgstreamer_android.so`
3. 通过 NDK CMake 编译 C 版 `libxue_hua_video_player.so`（`native/` + JNI）

伞形库（静态链接的全部 GStreamer + 插件）与 `libc++_shared.so` 输出到
`android/build/gstreamer/jniLibs/<abi>/` 并打进插件 AAR。**首次构建需要网络**；后续构建复用缓存
`~/Library/Caches/xue_hua_video_player/gstreamer/android/<version>/`。

环境变量：

| 变量 | 用途 |
| --- | --- |
| `GST_VER` | GStreamer 版本（默认 `1.28.4`） |
| `GSTREAMER_ROOT_ANDROID` | SDK 根目录（已手动解压时可跳过自动下载） |
| `XUE_HUA_GSTREAMER_ROOT` | 自定义 SDK/缓存根目录的别名 |

GStreamer 的 Android 运行时会在进程启动时由 `GStreamerInitProvider`（插件
`android/src/main/java/` 下的一个 `ContentProvider`）自动初始化：它执行
`System.loadLibrary("gstreamer_android")`（使该库的 `JNI_OnLoad` 捕获 JavaVM）并调用
`GStreamer.init(context)`（设置应用的 `Context`/`ClassLoader`）。这一步是必需的，`androidmedia`
的 MediaCodec 解码器只有在此之后才能扫描/注册；否则播放会报 `not-linked` /
`No streams to output`。因此 Rust 核心在 Android 上不再自行注册插件（那会在 Java 初始化之前、
且没有 JavaVM 的情况下运行）。

使用方默认会构建全部四种 ABI。若要减小 APK 体积，可收窄 `abiFilters`（见
[权限与各平台配置](#android)）或发布 App Bundle。

#### 自定义插件集

在 [`android/gstreamer_build/jni/Android.mk`](android/gstreamer_build/jni/Android.mk) 中编辑
`GSTREAMER_PLUGINS` 以增删编解码器，然后重新构建；下次 Android 构建会自动重新生成伞形库。

#### 手动下载 SDK / 重建伞形库（可选）

Gradle 会自动运行
[`android/scripts/ensure_gstreamer_android.sh`](android/scripts/ensure_gstreamer_android.sh) 与
[`android/scripts/build_gstreamer_umbrella.sh`](android/scripts/build_gstreamer_umbrella.sh)。
也可手动执行：

```bash
sh android/scripts/ensure_gstreamer_android.sh
sh android/scripts/build_gstreamer_umbrella.sh \
  "$HOME/Library/Android/sdk/ndk/<ndk-version>" \
  /tmp/gstreamer-jniLibs \
  arm64-v8a armeabi-v7a x86 x86_64
```

## 架构

```
Dart:  XueHuaPlayerController ──FFI──► FfiPlayerCommandPort (xhvp_player_*)
       XueHuaVideoView (Texture) ◄──native texture── GStreamer sink
C:     native/ playbin3 ─► appsink（Apple/桌面）或 glimagesink（Android）
                     │ 总线 ─► XhvpEventCallback ─► Dart Stream
```

- 解码：`playbin3` + 平台 sink（`appsink` 或 `glimagesink`）。
- 渲染：Flutter `Texture`；Android 为 `SurfaceProducer` + VideoOverlay；Apple/桌面经 C ABI（`xhvp_texture_*`）拉取 BGRA 帧。
- 控制面：Dart FFI → 窄 `xhvp_player_*` API（见 `native/include/xhvp_player.h`）。

### 重新生成 FFI 绑定

修改 `native/include/xhvp_player.h` 后：

```bash
dart run ffigen --config ffigen.yaml
```

### GStreamer 上游补丁

需要改 GStreamer C 源时，以 [Matkurban/gstreamer](https://github.com/Matkurban/gstreamer) 为补丁源（`XHVP_GSTREAMER_SRC`），见 [third_party/gstreamer.md](third_party/gstreamer.md)。

## 常见问题

- **Release 启动报 `Failed to lookup symbol 'xhvp_init'`（仅 iOS/macOS）：**
  全局 FFI 符号被剥离。见
  [Apple Release / FFI 符号](#apple-release--ffi-符号ios-与-macos)
  （Strip Style = Non-Global Symbols；CocoaPods 在 `pod install` 后通常会自动注入）。
- **iOS 启动出现 `g_dir_open_with_errno` / `g_filename_to_utf8` / ORC mmap 错误：**
  GLib/GStreamer 需要可写的 `HOME`/`XDG_*`/`GST_REGISTRY`，且 Hardened Runtime
  会阻止 ORC JIT。C 核心在 `gst_init` 前设置这些变量（`native/src/apple_env.c`，
  `ORC_CODE=backup`）。不要添加 `allow-jit`。
- **网络视频报 `Can't typefind stream` / `Stream doesn't contain enough data`（iOS）**：
  未配置 CA 数据库导致 TLS 握手失败。C 核心会注册 OpenSSL TLS 后端
  （`native/src/ios_tls.c`）并在 playbin `source-setup` 中放宽 `ssl-strict`。
- **APK 体积过大**：四种 Android ABI 各自携带较大的 GStreamer 运行时。可收窄 `abiFilters`，
  或发布按 ABI 拆分的 APK / App Bundle。
- **Android 首次构建失败 / 无网络**：首次构建会下载 GStreamer Android SDK。离线 CI 可预先
  设置 `GSTREAMER_ROOT_ANDROID`，或缓存 `~/Library/Caches/xue_hua_video_player/gstreamer/android/`。
- **Windows `pkg-config` 找不到 `glib-2.0`**：确认已安装**开发**文件，且 `PKG_CONFIG_PATH`
  指向 `...\1.0\msvc_x86_64\lib\pkgconfig`。
- **macOS 黑屏 / dyld 找不到 GStreamer**：确认
  `.app/Contents/Frameworks/GStreamer.framework` 已存在。SPM 下需在
  `macos/Podfile` 的 `post_install` 调用 `install_gstreamer_embed_script!`
  （见 [macOS](#macos)），再 `pod install` 并重新构建。沙盒需开启
  `network.client`；Homebrew 调试请设置
  `XUE_HUA_ALLOW_HOMEBREW_GSTREAMER=1`（上架必须用官方 Framework）。

## 维护者

- 作者：**Matkurban** &lt;3496354336@qq.com&gt;
- 仓库：<https://github.com/Matkurban/xue_hua_video_player>
- 问题反馈：<https://github.com/Matkurban/xue_hua_video_player/issues>

## 许可证

见 [LICENSE](LICENSE)。

[`signals`]: https://pub.dev/packages/signals
