#!/usr/bin/env bash
# Builds libgstreamer_android.so (umbrella) for the requested Android ABIs via
# ndk-build, then installs into the SDK lib dirs (Rust link) and the Gradle
# jniLibs output directory (runtime packaging).
#
# Usage:
#   build_gstreamer_umbrella.sh <ndk_path> <output_jnilibs_dir> <abi> [abi...]
#
# Environment:
#   GSTREAMER_ROOT_ANDROID  — SDK root (see gstreamer_paths.sh)
#   GST_VER                 — GStreamer version (default 1.28.4)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_ANDROID_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
GSTREAMER_BUILD_DIR="${PLUGIN_ANDROID_DIR}/gstreamer_build"

# shellcheck source=gstreamer_paths.sh
source "${SCRIPT_DIR}/gstreamer_paths.sh"

if [[ $# -lt 3 ]]; then
  echo "usage: $0 <ndk_path> <output_jnilibs_dir> <abi> [abi...]" >&2
  exit 1
fi

NDK_PATH="$1"
OUTPUT_JNILIBS_DIR="$2"
shift 2
REQUESTED_ABIS=("$@")

if [[ ! -d "${NDK_PATH}" ]]; then
  echo "error: NDK not found at ${NDK_PATH}" >&2
  exit 1
fi

NDK_BUILD="${NDK_PATH}/ndk-build"
if [[ ! -x "${NDK_BUILD}" ]]; then
  echo "error: ndk-build not found at ${NDK_BUILD}" >&2
  exit 1
fi

# Gradle ABI name -> GStreamer SDK ABI folder name
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

ABI_FILTER=""
for abi in "${REQUESTED_ABIS[@]}"; do
  if [[ -n "${ABI_FILTER}" ]]; then
    ABI_FILTER="${ABI_FILTER} "
  fi
  ABI_FILTER="${ABI_FILTER}${abi}"
done

echo "[xue_hua_video_player] Building GStreamer umbrella for ABIs: ${ABI_FILTER}"
echo "[xue_hua_video_player] GSTREAMER_ROOT_ANDROID=${GSTREAMER_ROOT_ANDROID}"

(
  cd "${GSTREAMER_BUILD_DIR}"
  GSTREAMER_ROOT_ANDROID="${GSTREAMER_ROOT_ANDROID}" \
    "${NDK_BUILD}" \
    NDK_PROJECT_PATH=. \
    NDK_APPLICATION_MK=jni/Application.mk \
  APP_ABI="${ABI_FILTER}" \
    -j"$(getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)"
)

for abi in "${REQUESTED_ABIS[@]}"; do
  sdk_abi="$(sdk_abi_for "${abi}")"
  src_dir="${GSTREAMER_BUILD_DIR}/libs/${abi}"
  umbrella="${src_dir}/libgstreamer_android.so"
  cxx_shared="${src_dir}/libc++_shared.so"

  if [[ ! -f "${umbrella}" ]]; then
    echo "error: ndk-build did not produce ${umbrella}" >&2
    exit 1
  fi
  if [[ ! -f "${cxx_shared}" ]]; then
    echo "error: ndk-build did not produce ${cxx_shared}" >&2
    exit 1
  fi

  sdk_lib_dir="${GSTREAMER_ROOT_ANDROID}/${sdk_abi}/lib"
  mkdir -p "${sdk_lib_dir}" "${OUTPUT_JNILIBS_DIR}/${abi}"
  cp -f "${umbrella}" "${sdk_lib_dir}/"
  cp -f "${cxx_shared}" "${sdk_lib_dir}/"
  cp -f "${umbrella}" "${OUTPUT_JNILIBS_DIR}/${abi}/"
  cp -f "${cxx_shared}" "${OUTPUT_JNILIBS_DIR}/${abi}/"

  echo "[xue_hua_video_player] Installed umbrella for ${abi} -> ${sdk_lib_dir} and ${OUTPUT_JNILIBS_DIR}/${abi}"
done

echo "[xue_hua_video_player] GStreamer umbrella build complete"
