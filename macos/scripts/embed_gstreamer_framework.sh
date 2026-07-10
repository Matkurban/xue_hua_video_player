#!/usr/bin/env bash
# Copies the runtime GStreamer.framework snapshot into the host .app and re-signs it.
# Invoked from the Runner target via gstreamer_podfile_helper.rb (post_install).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=gstreamer_paths.sh
source "${SCRIPT_DIR}/gstreamer_paths.sh"

DEST_DIR="${TARGET_BUILD_DIR}/${FRAMEWORKS_FOLDER_PATH}"
DEST="${DEST_DIR}/GStreamer.framework"
SRC="${GSTREAMER_RUNTIME_FRAMEWORK_SRC}"

if [[ ! -d "${SRC}" ]] || [[ ! -f "${SRC}/Versions/1.0/lib/libgstreamer-1.0.0.dylib" ]]; then
  if [[ "${XUE_HUA_ALLOW_HOMEBREW_GSTREAMER:-}" == "1" ]]; then
    echo "warning: skipping GStreamer embed (XUE_HUA_ALLOW_HOMEBREW_GSTREAMER=1; Homebrew dev mode)"
    exit 0
  fi
  echo "[xue_hua_video_player] GStreamer runtime framework missing; running ensure..."
  sh "${SCRIPT_DIR}/ensure_gstreamer_macos.sh"
  source "${SCRIPT_DIR}/gstreamer_paths.sh"
  SRC="${GSTREAMER_RUNTIME_FRAMEWORK_SRC}"
fi

if [[ ! -d "${SRC}" ]]; then
  echo "error: GStreamer runtime framework not found at ${SRC}" >&2
  echo "Ensure failed. Check network connectivity or set XUE_HUA_GSTREAMER_ROOT." >&2
  exit 1
fi

if [[ -z "${EXPANDED_CODE_SIGN_IDENTITY:-}" ]] || [[ "${EXPANDED_CODE_SIGN_IDENTITY}" == "-" ]]; then
  echo "warning: EXPANDED_CODE_SIGN_IDENTITY not set; embedded libraries may fail MAS validation"
fi

echo "Embedding GStreamer.framework (runtime) from ${SRC} into ${DEST}"
mkdir -p "${DEST_DIR}"
rm -rf "${DEST}"
ditto "${SRC}" "${DEST}"
bash "${SCRIPT_DIR}/strip_gstreamer_runtime.sh" "${DEST}"
bash "${SCRIPT_DIR}/prune_gstreamer_plugins.sh" "${DEST}"
bash "${SCRIPT_DIR}/prune_gstreamer_orphan_dylibs.sh" "${DEST}"
bash "${SCRIPT_DIR}/thin_gstreamer_framework.sh" "${DEST}"

sign_if_needed() {
  local file="$1"
  local identity="${EXPANDED_CODE_SIGN_IDENTITY:-}"
  if [[ -n "${identity}" ]] && [[ "${identity}" != "-" ]]; then
    /usr/bin/codesign --force --sign "${identity}" --preserve-metadata=identifier,entitlements,flags "${file}"
  else
    /usr/bin/codesign --force --sign - "${file}" 2>/dev/null || true
  fi
}

identity="${EXPANDED_CODE_SIGN_IDENTITY:-}"
if [[ -n "${identity}" ]] && [[ "${identity}" != "-" ]]; then
  # Only regular files — skip symlinks (dangling ones break codesign).
  while IFS= read -r -d '' lib; do
    [[ -f "${lib}" ]] || continue
    sign_if_needed "${lib}"
  done < <(find "${DEST}" \( -name '*.dylib' -o -name '*.so' \) -type f -print0)
fi

sign_if_needed "${DEST}"

echo "Embedded GStreamer.framework ($(du -sh "${DEST}" | awk '{print $1}'))"
