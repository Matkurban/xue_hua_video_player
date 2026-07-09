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
  local mod_rs="${GST_PLUGINS_RS_CACHE}/net/reqwest/src/reqwesthttpsrc/mod.rs"
  local cargo_toml="${GST_PLUGINS_RS_CACHE}/net/reqwest/Cargo.toml"
  local patch_file="${GST_BUILD_DIR}/patches/reqwest-android-current-thread.patch"

  # Prefer the checked-in patch when the tree is still stock; otherwise apply
  # incremental edits for partially-patched caches.
  if ! grep -q "new_current_thread" "${imp}"; then
    if [[ -f "${patch_file}" ]] && command -v patch >/dev/null 2>&1; then
      (
        cd "${GST_PLUGINS_RS_CACHE}"
        patch -p1 --forward < "${patch_file}" || true
      )
    fi
  fi

  if grep -q "new_current_thread" "${imp}"; then
    echo "[xue_hua_video_player] reqwest Android patch already applied (imp.rs RUNTIME)"
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

  if grep -q "force_runtime_init" "${imp}"; then
    echo "[xue_hua_video_player] reqwest Android patch already applied (force_runtime_init)"
  else
    python3 - "${imp}" <<'PY'
import sys
from pathlib import Path
p = Path(sys.argv[1])
text = p.read_text()
needle = "});\n\nimpl ReqwestHttpSrc {"
insert = """});

/// Force Tokio RUNTIME LazyLock during plugin register (Android process start).
#[cfg(target_os = \"android\")]
pub(super) fn force_runtime_init() {
    let _ = &*RUNTIME;
}

impl ReqwestHttpSrc {"""
if needle not in text:
    raise SystemExit(f"insert point for force_runtime_init not found in {p}")
p.write_text(text.replace(needle, insert, 1))
PY
    echo "[xue_hua_video_player] Patched reqwesthttpsrc force_runtime_init"
  fi

  if grep -q "force_runtime_init" "${mod_rs}"; then
    echo "[xue_hua_video_player] reqwest Android patch already applied (mod.rs register)"
  else
    python3 - "${mod_rs}" <<'PY'
import sys
from pathlib import Path
p = Path(sys.argv[1])
text = p.read_text()
old = """pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register("""
new = """pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    // Claim pthread keys for Tokio while Bionic budget is still available
    // (GStreamer.init from ContentProvider), before SDK-heavy host code runs.
    #[cfg(target_os = \"android\")]
    imp::force_runtime_init();
    gst::Element::register("""
if old not in text:
    raise SystemExit(f"register() block not found in {p}")
p.write_text(text.replace(old, new, 1))
PY
    echo "[xue_hua_video_player] Patched reqwesthttpsrc register() for Android"
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
  local cc_path cxx_path ar_path
  local cc_env_underscores
  rust_target="$(rust_target_for_abi "${gradle_abi}")"
  sdk_abi="$(sdk_abi_for "${gradle_abi}")"
  clang_triple="$(clang_triple_for_abi "${gradle_abi}")"
  env_key="$(env_key_for_rust_target "${rust_target}")"
  # cc-rs looks for CC_<target> with the rustc triple using underscores / hyphens
  # (lowercase), not the Cargo CARGO_TARGET_* uppercase form.
  cc_env_underscores="${rust_target//-/_}"

  local gst_sysroot="${GSTREAMER_ROOT_ANDROID}/${sdk_abi}"
  local plugin_dir="${gst_sysroot}/lib/gstreamer-1.0"
  local out_a="${plugin_dir}/libgstreqwest.a"

  if [[ ! -f "${gst_sysroot}/lib/pkgconfig/gstreamer-1.0.pc" ]]; then
    echo "error: GStreamer Android SDK missing at ${gst_sysroot}" >&2
    exit 1
  fi

  cc_path="${NDK_BIN}/${clang_triple}-clang"
  cxx_path="${NDK_BIN}/${clang_triple}-clang++"
  ar_path="${NDK_BIN}/llvm-ar"
  if [[ ! -x "${cc_path}" ]]; then
    echo "error: NDK clang not found: ${cc_path}" >&2
    exit 1
  fi

  export PATH="${NDK_BIN}:${PATH}"
  # Cargo linker (uppercase triple with underscores).
  export "CARGO_TARGET_${env_key}_LINKER=${cc_path}"
  # cc-rs / ring: CC_<target_with_underscores> (hyphenated names are not valid
  # shell identifiers — do not export CC_armv7-linux-androideabi).
  export "CC_${cc_env_underscores}=${cc_path}"
  export "CXX_${cc_env_underscores}=${cxx_path}"
  export "AR_${cc_env_underscores}=${ar_path}"
  export TARGET_CC="${cc_path}"
  export TARGET_CXX="${cxx_path}"
  export TARGET_AR="${ar_path}"
  export CC="${cc_path}"
  export CXX="${cxx_path}"
  export AR="${ar_path}"
  export PKG_CONFIG_ALLOW_CROSS=1
  export PKG_CONFIG_SYSROOT_DIR="${gst_sysroot}"
  export PKG_CONFIG_LIBDIR="${gst_sysroot}/lib/pkgconfig"
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR}"

  echo "[xue_hua_video_player] Building libgstreqwest.a for ${gradle_abi} (${rust_target})..."
  echo "[xue_hua_video_player]   CC=${cc_path}"
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
