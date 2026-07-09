#!/usr/bin/env bash
# Cross-compiles gst-plugin-reqwest 0.15.2 with Android current_thread Tokio and
# installs libgstreqwest.a into the GStreamer Android SDK (per ABI).
#
# Usage:
#   build_reqwest_plugin_android.sh <ndk_path> <abi> [abi...]
#
# Gradle ABI names: arm64-v8a, armeabi-v7a, x86, x86_64
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_ANDROID_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
GST_BUILD_DIR="${PLUGIN_ANDROID_DIR}/gstreamer_build"

# shellcheck source=gstreamer_paths.sh
source "${SCRIPT_DIR}/gstreamer_paths.sh"

GST_PLUGINS_RS_VER="${GST_PLUGINS_RS_VER:-0.15.2}"
GST_PLUGINS_RS_CACHE="${GST_BUILD_DIR}/.cache/gst-plugins-rs-${GST_PLUGINS_RS_VER}"
CARGO_TARGET_DIR="${GST_PLUGINS_RS_CACHE}/target"

if [[ $# -lt 2 ]]; then
  echo "usage: $0 <ndk_path> <abi> [abi...]" >&2
  exit 1
fi

NDK_PATH="$1"
shift
REQUESTED_ABIS=("$@")

NDK_BUILD_PREBUILT="${NDK_PATH}/toolchains/llvm/prebuilt"
if [[ ! -d "${NDK_BUILD_PREBUILT}" ]]; then
  echo "error: NDK llvm prebuilt not found under ${NDK_PATH}" >&2
  exit 1
fi
NDK_BIN="$(find "${NDK_BUILD_PREBUILT}" -maxdepth 2 -type d -name bin | head -1)"
if [[ -z "${NDK_BIN}" || ! -d "${NDK_BIN}" ]]; then
  echo "error: could not locate NDK bin directory" >&2
  exit 1
fi

rust_target_for_abi() {
  case "$1" in
    arm64-v8a) echo aarch64-linux-android ;;
    armeabi-v7a) echo armv7-linux-androideabi ;;
    x86) echo i686-linux-android ;;
    x86_64) echo x86_64-linux-android ;;
    *)
      echo "error: unsupported ABI: $1" >&2
      exit 1
      ;;
  esac
}

sdk_abi_for() {
  case "$1" in
    arm64-v8a) echo arm64 ;;
    armeabi-v7a) echo armv7 ;;
    x86) echo x86 ;;
    x86_64) echo x86_64 ;;
    *)
      echo "error: unsupported ABI: $1" >&2
      exit 1
      ;;
  esac
}

clang_triple_for_abi() {
  case "$1" in
    arm64-v8a) echo aarch64-linux-android34 ;;
    armeabi-v7a) echo armv7a-linux-androideabi34 ;;
    x86) echo i686-linux-android34 ;;
    x86_64) echo x86_64-linux-android34 ;;
    *)
      echo "error: unsupported ABI: $1" >&2
      exit 1
      ;;
  esac
}

env_key_for_rust_target() {
  case "$1" in
    aarch64-linux-android) echo AARCH64_LINUX_ANDROID ;;
    armv7-linux-androideabi) echo ARMV7_LINUX_ANDROIDEABI ;;
    i686-linux-android) echo I686_LINUX_ANDROID ;;
    x86_64-linux-android) echo X86_64_LINUX_ANDROID ;;
    *)
      echo "error: unsupported rust target: $1" >&2
      exit 1
      ;;
  esac
}

apply_reqwest_android_patch() {
  local imp="${GST_PLUGINS_RS_CACHE}/net/reqwest/src/reqwesthttpsrc/imp.rs"
  local cargo_toml="${GST_PLUGINS_RS_CACHE}/net/reqwest/Cargo.toml"

  if grep -q "new_current_thread" "${imp}"; then
    echo "[xue_hua_video_player] reqwest Android patch already applied (imp.rs)"
  else
    python3 - "${imp}" <<'PY'
import sys
from pathlib import Path
p = Path(sys.argv[1])
text = p.read_text()
old = """static RUNTIME: LazyLock<runtime::Runtime> = LazyLock::new(|| {
    runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(1)
        .build()
        .unwrap()
});"""
new = """static RUNTIME: LazyLock<runtime::Runtime> = LazyLock::new(|| {
    #[cfg(target_os = \"android\")]
    {
        runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }
    #[cfg(not(target_os = \"android\"))]
    {
        runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(1)
            .build()
            .unwrap()
    }
});"""
if old not in text:
    raise SystemExit(f"RUNTIME block not found in {p}")
p.write_text(text.replace(old, new))
PY
    echo "[xue_hua_video_player] Patched reqwesthttpsrc RUNTIME for Android"
  fi

  if grep -q 'features = \["time", "rt"\]' "${cargo_toml}"; then
    echo "[xue_hua_video_player] reqwest Android patch already applied (Cargo.toml)"
  else
    sed -i.bak 's/features = \["time", "rt-multi-thread"\]/features = ["time", "rt"]/' "${cargo_toml}"
    rm -f "${cargo_toml}.bak"
    echo "[xue_hua_video_player] Patched reqwest tokio features for Android"
  fi
}

ensure_gst_plugins_rs_source() {
  if [[ ! -d "${GST_PLUGINS_RS_CACHE}/.git" ]]; then
    echo "[xue_hua_video_player] Cloning gst-plugins-rs ${GST_PLUGINS_RS_VER}..."
    git clone --depth 1 --branch "${GST_PLUGINS_RS_VER}" \
      https://github.com/GStreamer/gst-plugins-rs.git "${GST_PLUGINS_RS_CACHE}"
  fi

  apply_reqwest_android_patch
}

build_reqwest_for_abi() {
  local gradle_abi="$1"
  local rust_target sdk_abi clang_triple env_key
  rust_target="$(rust_target_for_abi "${gradle_abi}")"
  sdk_abi="$(sdk_abi_for "${gradle_abi}")"
  clang_triple="$(clang_triple_for_abi "${gradle_abi}")"
  env_key="$(env_key_for_rust_target "${rust_target}")"

  local gst_sysroot="${GSTREAMER_ROOT_ANDROID}/${sdk_abi}"
  local plugin_dir="${gst_sysroot}/lib/gstreamer-1.0"
  local out_a="${plugin_dir}/libgstreqwest.a"

  if [[ ! -f "${gst_sysroot}/lib/pkgconfig/gstreamer-1.0.pc" ]]; then
    echo "error: GStreamer Android SDK missing at ${gst_sysroot}" >&2
    exit 1
  fi

  export PATH="${NDK_BIN}:${PATH}"
  export "CC_${env_key}=${clang_triple}-clang"
  export "CXX_${env_key}=${clang_triple}-clang"
  export "AR_${env_key}=llvm-ar"
  export "CARGO_TARGET_${env_key}_LINKER=${clang_triple}-clang"
  export PKG_CONFIG_ALLOW_CROSS=1
  export PKG_CONFIG_SYSROOT_DIR="${gst_sysroot}"
  export PKG_CONFIG_LIBDIR="${gst_sysroot}/lib/pkgconfig"
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR}"

  echo "[xue_hua_video_player] Building libgstreqwest.a for ${gradle_abi} (${rust_target})..."
  (
    cd "${GST_PLUGINS_RS_CACHE}"
    cargo rustc -p gst-plugin-reqwest --release --target "${rust_target}" --crate-type staticlib
  )

  local built_a="${CARGO_TARGET_DIR}/${rust_target}/release/libgstreqwest.a"
  if [[ ! -f "${built_a}" ]]; then
    echo "error: cargo did not produce ${built_a}" >&2
    exit 1
  fi

  mkdir -p "${plugin_dir}"
  cp -f "${built_a}" "${out_a}"
  echo "[xue_hua_video_player] Installed ${out_a}"
}

ensure_gst_plugins_rs_source

for abi in "${REQUESTED_ABIS[@]}"; do
  build_reqwest_for_abi "${abi}"
done

echo "[xue_hua_video_player] Patched libgstreqwest.a ready for all requested ABIs"
