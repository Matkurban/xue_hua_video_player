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

echo "[xue_hua_video_player] Building patched libgstreqwest.a (Android current_thread Tokio)..."
"${SCRIPT_DIR}/build_reqwest_plugin_android.sh" "${NDK_PATH}" "${REQUESTED_ABIS[@]}"

(
  cd "${GSTREAMER_BUILD_DIR}"
  GSTREAMER_ROOT_ANDROID="${GSTREAMER_ROOT_ANDROID}" \
    "${NDK_BUILD}" \
    NDK_PROJECT_PATH=. \
    NDK_APPLICATION_MK=jni/Application.mk \
  APP_ABI="${ABI_FILTER}" \
    -j"$(getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)"
)

# Fail the build if the umbrella still embeds multi-thread Tokio (unpatched
# reqwesthttpsrc). Patched Android builds must use current_thread only.
#
# Under `set -o pipefail`, `strings | grep` fails with SIGPIPE whenever grep
# exits early after a match. Disable pipefail inside a subshell for the probe.
so_has_symbol() {
  local so_path="$1"
  local pattern="$2"
  (
    set +o pipefail
    strings "${so_path}" | grep -F "${pattern}" >/dev/null
  )
}

# Verify against the unstripped ndk-build output. 32-bit strip drops many Rust
# mangled names from libs/<abi>/, and current_thread Tokio still embeds
# BlockingPool type strings — so do not require absence of BlockingPool.
verify_reqwest_tokio_current_thread() {
  local abi="$1"
  local unstripped="${GSTREAMER_BUILD_DIR}/gst-android-build/${abi}/libgstreamer_android.so"
  local label="gst-android-build/${abi}"

  if [[ ! -f "${unstripped}" ]]; then
    echo "error: ${label}: unstripped umbrella missing" >&2
    exit 1
  fi
  if ! so_has_symbol "${unstripped}" 'Builder18new_current_thread'; then
    echo "error: ${label}: missing tokio new_current_thread (reqwest patch not linked?)" >&2
    echo "  path: ${unstripped}" >&2
    exit 1
  fi
  if so_has_symbol "${unstripped}" 'Builder16new_multi_thread'; then
    echo "error: ${label}: still contains tokio new_multi_thread (stale/unpatched reqwest)" >&2
    echo "  path: ${unstripped}" >&2
    exit 1
  fi
  echo "[xue_hua_video_player] ${label}: reqwest Tokio = current_thread"
}

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

  verify_reqwest_tokio_current_thread "${abi}"

  sdk_lib_dir="${GSTREAMER_ROOT_ANDROID}/${sdk_abi}/lib"
  mkdir -p "${sdk_lib_dir}" "${OUTPUT_JNILIBS_DIR}/${abi}"
  cp -f "${umbrella}" "${sdk_lib_dir}/"
  cp -f "${cxx_shared}" "${sdk_lib_dir}/"
  cp -f "${umbrella}" "${OUTPUT_JNILIBS_DIR}/${abi}/"
  cp -f "${cxx_shared}" "${OUTPUT_JNILIBS_DIR}/${abi}/"

  jni_umbrella="${OUTPUT_JNILIBS_DIR}/${abi}/libgstreamer_android.so"
  if ! cmp -s "${umbrella}" "${jni_umbrella}"; then
    echo "error: libs/${abi} and jniLibs/${abi} libgstreamer_android.so differ after copy" >&2
    exit 1
  fi

  # Stamp for Gradle up-to-date checks (stripped .so symbols are ABI-dependent).
  printf 'reqwest Tokio = current_thread\n' > "${OUTPUT_JNILIBS_DIR}/${abi}/.reqwest-tokio-current-thread"

  echo "[xue_hua_video_player] Installed umbrella for ${abi} -> ${sdk_lib_dir} and ${OUTPUT_JNILIBS_DIR}/${abi}"
done

echo "[xue_hua_video_player] GStreamer umbrella build complete"
