#!/usr/bin/env bash
# Refresh the checked-in icon and installer exports. Ordinary builds use those files directly.
set -euo pipefail
cd "$(dirname "$0")/.."

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
icons=src-tauri/icons

node scripts/render-brand-svgs.mjs

for flavor in release dev; do
  dir="$icons"
  tray="$icons/source/koeltype-tray.svg"
  if [[ "$flavor" == dev ]]; then
    dir="$icons/dev"
    tray="$dir/tray.svg"
  fi
  rsvg-convert -w 1024 -h 1024 "$dir/app-icon.svg" > "$tmp/$flavor.png"
  npm run tauri -- icon "$tmp/$flavor.png" --output "$tmp/$flavor"
  for asset in 32x32.png 64x64.png 128x128.png 128x128@2x.png icon.icns icon.ico icon.png; do
    cp "$tmp/$flavor/$asset" "$dir/$asset"
  done
  rsvg-convert -w 44 -h 44 "$tray" > "$dir/tray-template.png"
done

cp "$icons/128x128@2x.png" site/public/icon-256.png
cp "$icons/32x32.png" site/public/favicon-32.png
rsvg-convert -w 180 -h 180 "$icons/app-icon.svg" > site/public/apple-touch-icon.png
rsvg-convert "$icons/dmg/background.svg" > "$tmp/dmg.png"
sips -s format tiff "$tmp/dmg.png" --out "$icons/dmg/background.tiff" >/dev/null
