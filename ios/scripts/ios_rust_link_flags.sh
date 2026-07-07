#!/bin/sh
# Exports RUSTFLAGS so `cargo build` can link the static GStreamer iOS framework
# when producing libxue_hua_video_player.a (gstgl/opengl need UIKit/OpenGLES/etc.).
#
# Usage (from podspec or local dev):
#   export GSTREAMER_ROOT_IOS=~/Library/Developer/GStreamer/iPhone.sdk
#   . "$PODS_TARGET_SRCROOT/scripts/ios_rust_link_flags.sh"

set -e

GST_ROOT="${GSTREAMER_ROOT_IOS:-${HOME}/Library/Developer/GStreamer/iPhone.sdk}"

# Keep in sync with OTHER_LDFLAGS frameworks in xue_hua_video_player.podspec.
IOS_FRAMEWORKS="
  GStreamer
  UIKit
  QuartzCore
  CoreGraphics
  IOSurface
  Metal
  OpenGLES
  CoreFoundation
  CoreMedia
  CoreVideo
  CoreAudio
  AVFoundation
  AVFAudio
  AssetsLibrary
  AudioToolbox
  VideoToolbox
  Foundation
  Security
"

rustflags="-C link-arg=-F${GST_ROOT}"

for fw in $IOS_FRAMEWORKS; do
  rustflags="$rustflags -C link-arg=-framework -C link-arg=$fw"
done

for lib in iconv resolv z bz2 c++; do
  rustflags="$rustflags -C link-arg=-l${lib}"
done

export RUSTFLAGS="$rustflags"
