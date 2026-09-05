#!/bin/bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
tool="${ICON_COMPOSER_TOOL:-/Applications/Icon Composer.app/Contents/Executables/ictool}"
if [[ ! -x "$tool" ]]; then
  echo 'Install Icon Composer 2.x, or set ICON_COMPOSER_TOOL to its ictool executable.' >&2
  exit 1
fi
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT
# Use Apple's macOS mask/material rendering, then add the legacy icon canvas.
"$tool" "$root/assets/macos/Kitter.icon" --export-image \
  --output-file "$work_dir/render.png" --platform macOS --rendition Default \
  --width 1024 --height 1024 --scale 1 --design-generation 27
swift "$root/scripts/export-macos-icon.swift" \
  "$work_dir/render.png" "$root/assets/macos/app-icon.png"
