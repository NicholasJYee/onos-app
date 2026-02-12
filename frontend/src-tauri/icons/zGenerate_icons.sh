#!/usr/bin/env bash
set -e

# Usage:
#   ./zGenerate_icons.sh icon.png

SRC="$1"

# --- Exact PNG files ---

sips -z 16 16     "$SRC" --out icon_16x16.png
sips -z 32 32     "$SRC" --out icon_16x16@2x.png
sips -z 32 32     "$SRC" --out icon_32x32.png
sips -z 64 64     "$SRC" --out icon_32x32@2x.png
sips -z 128 128   "$SRC" --out icon_128x128.png
sips -z 256 256   "$SRC" --out icon_128x128@2x.png
sips -z 256 256   "$SRC" --out icon_256x256.png
sips -z 512 512   "$SRC" --out icon_256x256@2x.png
sips -z 512 512   "$SRC" --out icon_512x512.png
sips -z 1024 1024 "$SRC" --out icon_512x512@2x.png

sips -z 32 32     "$SRC" --out 32x32.png
sips -z 128 128   "$SRC" --out 128x128.png
sips -z 256 256   "$SRC" --out 128x128@2x.png

# --- icon.icns and app_icon.icns (temporary iconset, removed after) ---

mkdir icon.iconset

cp icon_16x16.png      icon.iconset/
cp icon_16x16@2x.png   icon.iconset/
cp icon_32x32.png      icon.iconset/
cp icon_32x32@2x.png   icon.iconset/
cp icon_128x128.png    icon.iconset/
cp icon_128x128@2x.png icon.iconset/
cp icon_256x256.png    icon.iconset/
cp icon_256x256@2x.png icon.iconset/
cp icon_512x512.png    icon.iconset/
cp icon_512x512@2x.png icon.iconset/

iconutil -c icns icon.iconset -o icon.icns
cp icon.icns app_icon.icns

rm -rf icon.iconset

# --- icon.ico and app_icon.ico ---

magick "$SRC" -background none -define icon:auto-resize=256,128,64,48,32,16 icon.ico
cp icon.ico app_icon.ico
