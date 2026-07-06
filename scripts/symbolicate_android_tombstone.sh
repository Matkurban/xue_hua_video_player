#!/usr/bin/env bash
# Symbolicate an Android tombstone / crash_dump64 backtrace for libxue_hua_video_player.so.
#
# Usage:
#   ./scripts/symbolicate_android_tombstone.sh <path-to-libxue_hua_video_player.so> <tombstone.txt>
#
# The .so is produced by a local cargokit build (see example/android/cargokit_options.yaml).
# Example after `flutter build apk --debug`:
#   SO=build/xue_hua_video_player/intermediates/merged_native_libs/debug/mergeDebugNativeLibs/out/lib/arm64-v8a/libxue_hua_video_player.so
#   ./scripts/symbolicate_android_tombstone.sh "$SO" tombstone.txt

set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "Usage: $0 <libxue_hua_video_player.so> <tombstone.txt>" >&2
  exit 1
fi

SO="$1"
TOMBSTONE="$2"

if [[ ! -f "$SO" ]]; then
  echo "Shared library not found: $SO" >&2
  exit 1
fi

if [[ ! -f "$TOMBSTONE" ]]; then
  echo "Tombstone file not found: $TOMBSTONE" >&2
  exit 1
fi

ADDR2LINE="${ADDR2LINE:-}"
if [[ -z "$ADDR2LINE" ]]; then
  if command -v llvm-addr2line >/dev/null 2>&1; then
    ADDR2LINE="$(command -v llvm-addr2line)"
  elif [[ -n "${ANDROID_NDK_HOME:-}" ]] && [[ -x "${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)/bin/llvm-addr2line" ]]; then
    ADDR2LINE="${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)/bin/llvm-addr2line"
  fi
fi

if [[ -z "$ADDR2LINE" ]] || [[ ! -x "$ADDR2LINE" ]]; then
  echo "llvm-addr2line not found. Set ADDR2LINE or ANDROID_NDK_HOME." >&2
  exit 1
fi

echo "Symbolicating frames from $TOMBSTONE using $SO"
grep 'libxue_hua_video_player.so' "$TOMBSTONE" | while read -r line; do
  pc="$(echo "$line" | sed -n 's/.*pc \([0-9a-fA-F]*\).*/\1/p')"
  if [[ -n "$pc" ]]; then
    echo "--- $line"
    "$ADDR2LINE" -e "$SO" -f -C "0x$pc"
  fi
done
