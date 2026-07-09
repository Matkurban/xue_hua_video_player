/// This is copied from Cargokit (which is the official way to use it currently)
/// Details: https://fzyzcjy.github.io/flutter_rust_bridge/manual/integrate/builtin

import 'dart:io';
import 'dart:isolate';
import 'dart:math' as math;

import 'package:collection/collection.dart';
import 'package:path/path.dart' as path;
import 'package:version/version.dart';

import 'target.dart';
import 'util.dart';

class AndroidEnvironment {
  AndroidEnvironment({
    required this.sdkPath,
    required this.ndkVersion,
    required this.minSdkVersion,
    required this.targetTempDir,
    required this.target,
  });

  static void clangLinkerWrapper(List<String> args) {
    final clang = Platform.environment['_CARGOKIT_NDK_LINK_CLANG'];
    if (clang == null) {
      throw Exception(
          "cargo-ndk rustc linker: didn't find _CARGOKIT_NDK_LINK_CLANG env var");
    }
    final target = Platform.environment['_CARGOKIT_NDK_LINK_TARGET'];
    if (target == null) {
      throw Exception(
          "cargo-ndk rustc linker: didn't find _CARGOKIT_NDK_LINK_TARGET env var");
    }

    runCommand(clang, [
      target,
      ...args,
    ]);
  }

  /// Full path to Android SDK.
  final String sdkPath;

  /// Full version of Android NDK.
  final String ndkVersion;

  /// Minimum supported SDK version.
  final int minSdkVersion;

  /// Target directory for build artifacts.
  final String targetTempDir;

  /// Target being built.
  final Target target;

  bool ndkIsInstalled() {
    final ndkPath = path.join(sdkPath, 'ndk', ndkVersion);
    final ndkPackageXml = File(path.join(ndkPath, 'package.xml'));
    return ndkPackageXml.existsSync();
  }

  void installNdk({
    required String javaHome,
  }) {
    final sdkManagerExtension = Platform.isWindows ? '.bat' : '';
    final sdkManager = path.join(
      sdkPath,
      'cmdline-tools',
      'latest',
      'bin',
      'sdkmanager$sdkManagerExtension',
    );

    log.info('Installing NDK $ndkVersion');
    runCommand(sdkManager, [
      '--install',
      'ndk;$ndkVersion',
    ], environment: {
      'JAVA_HOME': javaHome,
    });
  }

  Future<Map<String, String>> buildEnvironment() async {
    final hostArch = Platform.isMacOS
        ? "darwin-x86_64"
        : (Platform.isLinux ? "linux-x86_64" : "windows-x86_64");

    final ndkPath = path.join(sdkPath, 'ndk', ndkVersion);
    final toolchainPath = path.join(
      ndkPath,
      'toolchains',
      'llvm',
      'prebuilt',
      hostArch,
      'bin',
    );

    final minSdkVersion =
        math.max(target.androidMinSdkVersion!, this.minSdkVersion);

    final exe = Platform.isWindows ? '.exe' : '';

    final arKey = 'AR_${target.rust}';
    final arValue = ['${target.rust}-ar', 'llvm-ar', 'llvm-ar.exe']
        .map((e) => path.join(toolchainPath, e))
        .firstWhereOrNull((element) => File(element).existsSync());
    if (arValue == null) {
      throw Exception('Failed to find ar for $target in $toolchainPath');
    }

    final targetArg = '--target=${target.rust}$minSdkVersion';

    final ccKey = 'CC_${target.rust}';
    final ccValue = path.join(toolchainPath, 'clang$exe');
    final cfFlagsKey = 'CFLAGS_${target.rust}';
    final cFlagsValue = targetArg;

    final cxxKey = 'CXX_${target.rust}';
    final cxxValue = path.join(toolchainPath, 'clang++$exe');
    final cxxFlagsKey = 'CXXFLAGS_${target.rust}';
    final cxxFlagsValue = targetArg;

    final linkerKey =
        'cargo_target_${target.rust.replaceAll('-', '_')}_linker'.toUpperCase();

    final ranlibKey = 'RANLIB_${target.rust}';
    final ranlibValue = path.join(toolchainPath, 'llvm-ranlib$exe');

    final ndkVersionParsed = Version.parse(ndkVersion);
    final rustFlagsKey = 'CARGO_ENCODED_RUSTFLAGS';
    final rustFlagsValue = _libGccWorkaround(targetTempDir, ndkVersionParsed);

    final runRustTool =
        Platform.isWindows ? 'run_build_tool.cmd' : 'run_build_tool.sh';

    final packagePath = (await Isolate.resolvePackageUri(
            Uri.parse('package:build_tool/buildtool.dart')))!
        .toFilePath();
    final selfPath = path.canonicalize(path.join(
      packagePath,
      '..',
      '..',
      '..',
      runRustTool,
    ));

    // Make sure that run_build_tool is working properly even initially launched directly
    // through dart run.
    final toolTempDir =
        Platform.environment['CARGOKIT_TOOL_TEMP_DIR'] ?? targetTempDir;

    return {
      arKey: arValue,
      ccKey: ccValue,
      cfFlagsKey: cFlagsValue,
      cxxKey: cxxValue,
      cxxFlagsKey: cxxFlagsValue,
      ranlibKey: ranlibValue,
      rustFlagsKey: rustFlagsValue,
      linkerKey: selfPath,
      // Recognized by main() so we know when we're acting as a wrapper
      '_CARGOKIT_NDK_LINK_TARGET': targetArg,
      '_CARGOKIT_NDK_LINK_CLANG': ccValue,
      'CARGOKIT_TOOL_TEMP_DIR': toolTempDir,
      ..._gstreamerSystemDepsEnv(),
    };
  }

  /// Points the GStreamer/GLib `-sys` crates at the single umbrella
  /// `libgstreamer_android.so` (built via ndk-build; see
  /// `android/gstreamer_build`). We bypass pkg-config with
  /// `system-deps` env overrides because the Android GStreamer SDK ships only
  /// static libs, and because cargokit runs cargo from a working directory
  /// where `rust/.cargo/config.toml` is not discovered.
  ///
  /// The SDK root defaults to the user cache and can be overridden with
  /// GSTREAMER_ROOT_ANDROID. The umbrella `.so` must be built before the Rust
  /// link step (see `android/scripts/build_gstreamer_umbrella.sh`).
  Map<String, String> _gstreamerSystemDepsEnv() {
    final abi = const {
      'aarch64-linux-android': 'arm64',
      'armv7-linux-androideabi': 'armv7',
      'i686-linux-android': 'x86',
      'x86_64-linux-android': 'x86_64',
    }[target.rust];
    if (abi == null) {
      return {};
    }
    final gstVer = Platform.environment['GST_VER'] ?? '1.28.4';
    final sdkRoot = Platform.environment['GSTREAMER_ROOT_ANDROID'] ??
        _defaultGstreamerAndroidCacheRoot(gstVer);
    final libDir = path.join(sdkRoot, abi, 'lib');
    final umbrella = File(path.join(libDir, 'libgstreamer_android.so'));
    if (!umbrella.existsSync()) {
      throw Exception(
        'GStreamer Android umbrella library not found at ${umbrella.path}. '
        'Run the Android build (which downloads the SDK and runs ndk-build) or '
        'set GSTREAMER_ROOT_ANDROID to a GStreamer Android SDK root with '
        'libgstreamer_android.so installed per ABI.',
      );
    }
    const pkgs = [
      'GLIB_2_0',
      'GOBJECT_2_0',
      'GIO_2_0',
      'GSTREAMER_1_0',
      'GSTREAMER_BASE_1_0',
      'GSTREAMER_APP_1_0',
      'GSTREAMER_VIDEO_1_0',
    ];
    final env = <String, String>{};
    for (final p in pkgs) {
      env['SYSTEM_DEPS_${p}_NO_PKG_CONFIG'] = '1';
      env['SYSTEM_DEPS_${p}_LIB'] = 'gstreamer_android';
      env['SYSTEM_DEPS_${p}_SEARCH_NATIVE'] = libDir;
    }
    return env;
  }

  static String _defaultGstreamerAndroidCacheRoot(String gstVer) {
    final home = Platform.environment['HOME'] ?? '';
    if (Platform.isMacOS) {
      return path.join(
        home,
        'Library',
        'Caches',
        'xue_hua_video_player',
        'gstreamer',
        'android',
        gstVer,
      );
    }
    if (Platform.isWindows) {
      final localAppData =
          Platform.environment['LOCALAPPDATA'] ?? path.join(home, 'AppData', 'Local');
      return path.join(
        localAppData,
        'xue_hua_video_player',
        'gstreamer',
        'android',
        gstVer,
      );
    }
    final xdgCache = Platform.environment['XDG_CACHE_HOME'] ?? path.join(home, '.cache');
    return path.join(
      xdgCache,
      'xue_hua_video_player',
      'gstreamer',
      'android',
      gstVer,
    );
  }

  // Workaround for libgcc missing in NDK23, inspired by cargo-ndk
  String _libGccWorkaround(String buildDir, Version ndkVersion) {
    final workaroundDir = path.join(
      buildDir,
      'cargokit',
      'libgcc_workaround',
      '${ndkVersion.major}',
    );
    Directory(workaroundDir).createSync(recursive: true);
    if (ndkVersion.major >= 23) {
      File(path.join(workaroundDir, 'libgcc.a'))
          .writeAsStringSync('INPUT(-lunwind)');
    } else {
      // Other way around, untested, forward libgcc.a from libunwind once Rust
      // gets updated for NDK23+.
      File(path.join(workaroundDir, 'libunwind.a'))
          .writeAsStringSync('INPUT(-lgcc)');
    }

    var rustFlags = Platform.environment['CARGO_ENCODED_RUSTFLAGS'] ?? '';
    if (rustFlags.isNotEmpty) {
      rustFlags = '$rustFlags\x1f';
    }
    rustFlags = '$rustFlags-L\x1f$workaroundDir';
    return rustFlags;
  }
}
