#!/bin/sh
# Exports CARGO_ENCODED_RUSTFLAGS so `cargo build` can link the static GStreamer iOS framework
# when producing libxue_hua_video_player.a (gstgl/opengl need UIKit/OpenGLES/etc.).
#
# Usage (from podspec or local dev):
#   export GSTREAMER_ROOT_IOS=~/Library/Developer/GStreamer/iPhone.sdk
#   . "$PODS_TARGET_SRCROOT/scripts/ios_rust_link_flags.sh"

set -e

GST_ROOT="${GSTREAMER_ROOT_IOS:-${HOME}/Library/Developer/GStreamer/iPhone.sdk}"

# Keep in sync with OTHER_LDFLAGS frameworks in xue_hua_video_player.podspec.
IOS_FRAMEWORKS="GStreamer UIKit QuartzCore CoreGraphics IOSurface Metal OpenGLES CoreFoundation CoreMedia CoreVideo CoreAudio AVFoundation AVFAudio AssetsLibrary AudioToolbox VideoToolbox Foundation Security"

# CARGO_ENCODED_RUSTFLAGS uses ASCII unit separator (0x1f) between rustc args so
# framework names are not split on spaces (RUSTFLAGS would treat "GStreamer UIKit" as two args).
_sep=''
encoded=""

append_flag() {
  if [ -n "$encoded" ]; then
    encoded="${encoded}${_sep}$1"
  else
    encoded="$1"
  fi
}

append_link_arg() {
  append_flag "-C"
  append_flag "link-arg=$1"
}

append_link_arg "-F${GST_ROOT}"

case "${CARGO_BUILD_TARGET:-}" in
  *ios-sim*)
    append_link_arg "-mios-simulator-version-min=13.0"
    ;;
  *)
    append_link_arg "-miphoneos-version-min=13.0"
    ;;
esac

for fw in $IOS_FRAMEWORKS; do
  append_link_arg "-framework"
  append_link_arg "$fw"
done

for lib in iconv resolv z bz2 c++; do
  append_link_arg "-l${lib}"
done

export CARGO_ENCODED_RUSTFLAGS="$encoded"
unset RUSTFLAGS
