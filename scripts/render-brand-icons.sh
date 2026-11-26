#!/usr/bin/env bash
# Refresh the checked-in icon and installer exports. Ordinary builds use those files directly.
set -euo pipefail
cd "$(dirname "$0")/.."

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
icons=src-tauri/icons

# Dev builds use a lower amber tab so the silhouette stays identical at Dock sizes.
sed '/<\/svg>/i\
  <rect x="365" y="799" width="294" height="35" rx="17" fill="#a65c18"/>\
' "$icons/app-icon.svg" > "$icons/dev/app-icon.svg"
sed '/<\/svg>/i\
  <circle cx="18" cy="17.5" r="1.7" fill="#000"/>\
' "$icons/source/koeltype-tray.svg" > "$icons/dev/tray.svg"

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
cp "$icons/128x128@2x.png" src/assets/koeltype-icon.png
cp "$icons/32x32.png" site/public/favicon-32.png
cp "$icons/app-icon.svg" site/public/favicon.svg
rsvg-convert -w 180 -h 180 "$icons/app-icon.svg" > site/public/apple-touch-icon.png
rsvg-convert "$icons/dmg/background.svg" > "$tmp/dmg.png"
sips -s format tiff "$tmp/dmg.png" --out "$icons/dmg/background.tiff" >/dev/null
