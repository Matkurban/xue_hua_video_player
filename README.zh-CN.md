# xue_hua_video_player

[English](README.md) | 简体中文

一个跨平台的 Flutter **视频播放器**插件：底层通过 Rust（[`flutter_rust_bridge`]）驱动
**GStreamer** 解码本地与网络视频，并借助 [`irondash_texture`] 把画面渲染到 Flutter 的
**外部纹理（external texture）**（在可行的平台上做到 GPU 零拷贝）。

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
- [API 说明](#api-说明)
- [各平台的 GStreamer 原生环境配置](#各平台的-gstreamer-原生环境配置)
- [预编译二进制](#预编译二进制)
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
- GPU 纹理渲染（不会把每一帧拷贝回 Dart 侧）。

## 平台支持

| 平台 | 最低版本 | 架构 | GStreamer 运行时 |
| --- | --- | --- | --- |
| Android | API 24（7.0） | `arm64-v8a`、`armeabi-v7a`、`x86`、`x86_64` | 已内置于插件（无需配置） |
| iOS | 13.0 | 真机 `arm64`（不支持模拟器） | GStreamer iOS SDK（静态 framework） |
| macOS | 10.13 | x86_64 / arm64 | Homebrew 或 `GStreamer.framework` |
| Windows | 10+ | x86_64 | GStreamer MSVC 运行时 |
| Linux | — | x86_64 | 系统 GStreamer + GTK 3 |

> 不支持 Apple Silicon 的 iOS **模拟器**，因为官方预编译 iOS SDK 不提供 arm64 模拟器切片。

## 安装

本包通过 Git 分发。在应用的 `pubspec.yaml` 中添加：

```yaml
dependencies:
  xue_hua_video_player:
    git:
      url: https://github.com/Matkurban/xue_hua_video_player.git
      ref: v1.0.0
```

然后执行：

```bash
flutter pub get
```

默认会以**预编译二进制**方式获取 Rust 核心，因此你的机器**无需**安装 Rust 工具链
（详见[预编译二进制](#预编译二进制)）。桌面端与 iOS 仍需在构建/运行时提供 GStreamer
SDK；Android 已内置全部依赖（见下文各平台说明）。

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

首次使用资源（asset）时会把它复制到临时文件，因为 GStreamer 只能读取文件系统路径和
URL，无法直接读取 Flutter 资源包。

## 在应用中集成（请先阅读）

1. **在创建任何控制器之前，先调用一次 `XueHuaVideoPlayer.initialize()`**（通常放在
   `main()` 里、`WidgetsFlutterBinding.ensureInitialized()` 之后）。该方法幂等，热重启后
   再次调用也安全。
2. **每个视频画面创建并 `initialize()` 一个 `XueHuaPlayerController`。** 控制器持有一个
   原生播放器和一张 GPU 纹理，二者在 `initialize()` 时创建。
3. **画面销毁时务必调用 `dispose()`** —— 它会停止管线、取消事件流，并在平台线程上释放
   纹理。忘记释放会泄漏原生管线。
4. **在 `SignalBuilder`/`Watch` 内读取状态。** 所有状态字段都是 `ReadonlySignal`；在响应式
   builder 之外读取 `.value` 不会在其变化时触发重建。
5. **Android 已内置全部四种 ABI**（`arm64-v8a`、`armeabi-v7a`、`x86`、`x86_64`）。可按需用
   `abiFilters` 收窄以减小 APK 体积（见下文）。
6. **iOS/macOS/Windows/Linux 在构建时需要 GStreamer SDK**（运行时需要其运行库）。见
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

插件已内置全部四种 ABI（`arm64-v8a`、`armeabi-v7a`、`x86`、`x86_64`），因此**无需**配置
`abiFilters`。如果想缩小 APK 到你实际发布的 ABI，可在 `android/app/build.gradle(.kts)` 中
收窄：

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

建议使用按 ABI 拆分的 APK 或 Android App Bundle，让每台设备只下载自己的 ABI —— 内置的
GStreamer 运行时较大（每个 ABI 约 13–18 MB）。

> 插件为完整性也提供了 `x86`（32 位）库，但当前 Flutter 已不再构建 32 位 x86 应用，因此
> 实际上 Flutter 应用真正打包的是 `arm64-v8a`、`armeabi-v7a` 与 `x86_64`。

说明：

- 插件的 `AndroidManifest.xml` 已设置 `android:extractNativeLibs="true"`，会合并进你的应用，
  确保内置的 `libgstreamer_android.so` 被解压到磁盘供动态加载器使用。
- 部分传递依赖需要较新的 `compileSdk`。若 AAR 元数据校验失败，可在所有子工程强制
  `compileSdk = 36`（示例工程在其 `android/build.gradle.kts` 中就是这样做的）。

### iOS

- **最低部署版本：iOS 13.0。** 仅支持真机 `arm64`（不支持模拟器）。
- GStreamer 使用自身的 socket + OpenSSL 进行网络通信，**不**走 `NSURLSession`，因此 App
  Transport Security（ATS）**不会**拦截播放，GStreamer 的流也**无需**配置
  `NSAppTransportSecurity`（`http://` 与 `https://` 均可播放）。
- 静态 iOS framework 的插件与 TLS 后端在 Rust 核心中注册（`rust/src/player.rs`，
  `#[cfg(target_os = "ios")]`）。为兼容那些不在内置 CA 链中的主机，HTTPS 证书校验被有意
  放宽（`ssl-strict = false`）—— 见[HTTPS 安全提示](#https-安全提示)。

### macOS

播放网络视频需要**对外网络**授权。请在 `macos/Runner/DebugProfile.entitlements` 和
`Release.entitlements` 中都加入：

```xml
<key>com.apple.security.network.client</key>
<true/>
```

如果保持 **App Sandbox 开启**，则必须把 GStreamer 的 dylib 打包进 `.app`，并加入：

```xml
<key>com.apple.security.cs.disable-library-validation</key>
<true/>
```

示例工程为方便开发**关闭**了 App Sandbox，从而可直接加载 Homebrew 的 dylib；正式发布的
应用应打包 GStreamer 并保持沙盒开启。

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
| `initialize()` | 创建原生播放器 + 纹理，并订阅事件。 |
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
`textureId`、`initialized`。

`PlayerState`：`idle`、`ready`、`buffering`、`playing`、`paused`、`stopped`、
`completed`、`error`。

### `VideoSource`

- `VideoSource.network(String url)`
- `VideoSource.file(String path)`（接受普通路径或 `file://` URI）
- `VideoSource.asset(String assetKey)`

### `XueHuaVideoView`

一个 `StatelessWidget`，渲染控制器的纹理，并默认叠加一个自适应控制条。

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `controller` | 必填 | 要渲染的 `XueHuaPlayerController`。 |
| `fit` | `BoxFit.contain` | 视频的填充方式。 |
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

通过 Homebrew 安装（`macos/xue_hua_video_player.podspec` 默认使用）：

```bash
brew install pkg-config gstreamer gst-plugins-base gst-plugins-good \
  gst-plugins-bad gst-libav
```

如需改用官方 `GStreamer.framework`，将 `GSTREAMER_PKG_CONFIG_PATH` 设置为它的
`.../Versions/1.0/lib/pkgconfig`。

发布时你必须把 GStreamer 的 dylib 打包进 `.app` 并修正其加载路径；上面的 Homebrew 方式仅
用于本地开发。

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
2. `ios/xue_hua_video_player.podspec` 已为 Rust 交叉编译导出 `system-deps` 覆盖项与
   `PKG_CONFIG_ALLOW_CROSS=1`，并链接伞形 `-framework GStreamer`。
3. 连接真机后构建/运行：`flutter run -d <device>`（或用 `flutter build ios --no-codesign`
   验证构建）。

由于 iOS 的 `GStreamer.framework` 是静态的，其插件不会被自动发现。Rust 核心会注册播放所需
的插件与 OpenSSL TLS 后端（`rust/src/player.rs` 中的 `register_ios_static_plugins()` /
`register_ios_tls_backend()`，受 `#[cfg(target_os = "ios")]` 保护），并在 `gst::init()`
之前准备运行环境（`ORC_CODE=backup`、`HOME`/`TMPDIR`/`XDG_*`）。若需要尚未注册的元件，请把
它加入插件列表。

### Android（全部 ABI）

支持 `arm64-v8a`、`armeabi-v7a`、`x86`、`x86_64`。**使用方无需任何 GStreamer 配置**：插件已
内置完整 GStreamer 运行时。`android/src/main/jniLibs/<abi>/` 为每个 ABI 提供伞形
`libgstreamer_android.so`（静态链接的全部 GStreamer + 插件）与 `libc++_shared.so`；它们被打
进插件 AAR 并合并到应用。Rust 的 `libxue_hua_video_player.so` 按 ABI 以预编译二进制获取。Rust
核心自行注册静态插件（`gst_init_static_plugins()`），因此无需 `GStreamer.init` 之类的 Java 类。

使用方默认会构建全部四种 ABI。若要减小 APK 体积，可收窄 `abiFilters`（见
[权限与各平台配置](#android)）或发布 App Bundle。若你是需要重建伞形库的维护者，见下文
“重新生成内置 `.so`”。

#### 重新生成内置 `.so`（维护者）

只有需要从源码重建 Rust 库（而非使用预编译二进制）或重新生成伞形 `.so` 的维护者才需要
GStreamer Android SDK。

**1. 下载并解压 GStreamer Android SDK**（顶层是各 ABI 目录）：

```bash
curl -fLO https://gstreamer.freedesktop.org/data/pkg/android/1.28.4/gstreamer-1.0-android-universal-1.28.4.tar.xz
mkdir -p ~/Library/Developer/GStreamer/android/1.28.4
# 注意：不要用 --strip-components；压缩包顶层是 arm64/ armv7/ x86/ x86_64/
tar -xf gstreamer-1.0-android-universal-1.28.4.tar.xz \
  -C ~/Library/Developer/GStreamer/android/1.28.4
```

**2. 从 `android/gstreamer_build` 的配方为所有 ABI 构建 `libgstreamer_android.so`**
（`jni/Application.mk` 已设置 `APP_ABI := arm64-v8a armeabi-v7a x86 x86_64`；在其中的
`GSTREAMER_PLUGINS` 列表增删编解码器）：

```bash
cd android/gstreamer_build
export GSTREAMER_ROOT_ANDROID="$HOME/Library/Developer/GStreamer/android/1.28.4"
~/Library/Android/sdk/ndk/<ndk-version>/ndk-build \
  NDK_PROJECT_PATH=. NDK_APPLICATION_MK=jni/Application.mk -j4
# -> 每个 ABI 的 libs/<abi>/libgstreamer_android.so (+ libc++_shared.so)
```

**3. 把伞形 `.so` 安装到 `jniLibs`（提交入库、运行时打包）与 SDK 的各 ABI `lib` 目录（供 Rust
链接步骤使用）。** 注意 GStreamer SDK 用 `armv7`/`arm64` 目录名，而 jniLibs 用
`armeabi-v7a`/`arm64-v8a`：

```bash
GST=~/Library/Developer/GStreamer/android/1.28.4
declare -A SDK=( [arm64-v8a]=arm64 [armeabi-v7a]=armv7 [x86]=x86 [x86_64]=x86_64 )
for abi in arm64-v8a armeabi-v7a x86 x86_64; do
  cp libs/$abi/libgstreamer_android.so "$GST/${SDK[$abi]}/lib/"          # 供 Rust 链接
  mkdir -p ../src/main/jniLibs/$abi
  cp libs/$abi/libgstreamer_android.so libs/$abi/libc++_shared.so \
    ../src/main/jniLibs/$abi/                                            # 运行时
done
```

## 预编译二进制

Rust 核心使用 [cargokit] 构建，支持**预编译二进制**，使插件的使用方**无需** Rust 工具链。
使用方构建应用时，cargokit 会根据 Rust 源码（`rust/src/**`、`Cargo.toml`、`Cargo.lock`、
`cargokit.yaml`）计算 `crate-hash`，并从本仓库的 GitHub Releases 下载对应哈希的、经签名的
预编译库，而不是运行 `cargo`。

> 预编译二进制只免除对 **Rust 工具链**的依赖。产物仍引用 GStreamer 符号，因此
> iOS/macOS/Linux/Windows 使用方在链接/运行时仍需要 GStreamer SDK。Android 例外：插件内置了
> GStreamer 运行时，既不需要 Rust 也不需要 GStreamer SDK。

行为：

- **未安装** Rust 的使用方会自动使用预编译二进制。
- **已安装** Rust 的使用方默认从源码构建。若想强制走预编译路径，在应用 `pubspec.yaml` 旁
  新建 `cargokit_options.yaml`：

  ```yaml
  use_precompiled_binaries: true
  ```

相关配置位于 [`rust/cargokit.yaml`](rust/cargokit.yaml)（下载 URL 前缀，以及用于校验签名的
ed25519 **公钥**）。

### 发布（维护者）

预编译二进制由
[`.github/workflows/precompile_binaries.yml`](.github/workflows/precompile_binaries.yml)
在每次推送到 `main` 时产出（macOS x86_64/arm64、iOS arm64、Linux x86_64、Windows x86_64、
Android arm64-v8a/armeabi-v7a/x86/x86_64），并上传到标签为 `precompiled_<hash>` 的 release。

在 GitHub 仓库（`Matkurban/xue_hua_video_player`）中的一次性设置：

1. 生成签名密钥对：

   ```bash
   cd cargokit/build_tool
   dart run build_tool gen-key
   ```

2. 把**公钥**填入 `rust/cargokit.yaml`（`public_key:`）并提交。
3. 把**私钥**添加为仓库 secret `PRIVATE_KEY`（Settings -> Secrets and variables ->
   Actions）。切勿提交私钥。
4. 工作流使用内置 `GITHUB_TOKEN`（`contents: write`）创建 release 并上传产物。

每当 Rust 源码 / `Cargo.lock` 变化，crate 哈希随之变化，工作流会自动重新发布新的 release。请
保持 `rust/Cargo.lock` 提交在库中，以便 CI 与使用方的哈希一致。

[cargokit]: https://fzyzcjy.github.io/flutter_rust_bridge/manual/integrate/builtin

## 架构

```
Dart:  XueHuaPlayerController ──FRB 调用──► Rust API (rust/src/api/player.rs)
       XueHuaVideoView (Texture) ◄──帧数据── irondash 纹理
Rust:  GstPlayer  playbin3 ─► videoconvert ─► appsink (RGBA) ─► FrameBuffer
                     │ 总线消息 ─► StreamSink<PlayerEvent> ─► Dart
```

- 解码：`playbin3`，视频 sink 设为 `appsink`（包在 `videoconvert` bin 中），强制
  `video/x-raw,format=RGBA`、`max-buffers=1`、`drop=true`。
- 渲染：appsink 回调把每一帧拷贝到共享缓冲并调用 `mark_frame_available`；irondash 在光栅
  线程请求该帧。
- 纹理通过 irondash 的 run loop 在平台主线程创建，其 id 交给 Flutter 的 `Texture` 组件。

### 重新生成绑定

修改 Rust API 后：

```bash
flutter_rust_bridge_codegen generate
```

### 内置依赖补丁

`rust/vendor/irondash_texture` 是 `irondash_texture` 0.5.0 的本地副本，包含一处 macOS/iOS
修复：上游底层 `IOSurface` 使用 `bytesPerRow = width * 4`，在当前 Flutter 渲染器上不满足
Metal 的行对齐要求（`Could not create Metal texture from pixel buffer: CVReturn -6684`）。该
副本把 stride 对齐到 256 字节并逐行上传，通过 `rust/Cargo.toml` 的 `[patch.crates-io]` 接入。

## 常见问题

- **网络视频报 `Can't typefind stream`（iOS）**：未配置 CA 数据库导致 TLS 握手失败。本版本已
  注册 OpenSSL TLS 后端并放宽 `ssl-strict`；请确保使用 v1.0.0+。
- **APK 体积过大**：四种 Android ABI 各自内置了较大的 GStreamer 运行时。可收窄 `abiFilters`，
  或发布按 ABI 拆分的 APK / App Bundle。
- **Windows `pkg-config` 找不到 `glib-2.0`**：确认已安装**开发**文件，且 `PKG_CONFIG_PATH`
  指向 `...\1.0\msvc_x86_64\lib\pkgconfig`。
- **macOS 黑屏 / dylib 加载失败**：开启 `com.apple.security.network.client`，并在开发时关闭
  沙盒，或打包 dylib 并加入 `disable-library-validation`。

## 维护者

- 作者：**Matkurban** &lt;3496354336@qq.com&gt;
- 仓库：<https://github.com/Matkurban/xue_hua_video_player>
- 问题反馈：<https://github.com/Matkurban/xue_hua_video_player/issues>

## 许可证

见 [LICENSE](LICENSE)。

[`flutter_rust_bridge`]: https://pub.dev/packages/flutter_rust_bridge
[`irondash_texture`]: https://crates.io/crates/irondash_texture
[`signals`]: https://pub.dev/packages/signals
