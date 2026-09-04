#!/usr/bin/env bash

set -euo pipefail

if [ "$#" -ne 3 ]; then
  echo "Usage: $0 <Kitter.app> <background.png> <output.dmg>" >&2
  exit 2
fi

app_path="$1"
background_path="$2"
output_path="$3"
script_dir="$(cd "$(dirname "$0")" && pwd)"
settings_path="${script_dir}/dmg-settings.py"

test -d "$app_path"
test -f "$background_path"
test -f "$settings_path"

rm -f "$output_path"
uvx --from "dmgbuild==1.6.7" dmgbuild \
  -s "$settings_path" \
  -D "app=${app_path}" \
  -D "background=${background_path}" \
  "Kitter Installer" \
  "$output_path"

hdiutil verify "$output_path"

mount_root="$(mktemp -d "${TMPDIR:-/tmp}/kitter-dmg.XXXXXX")"
cleanup() {
  hdiutil detach "$mount_root" >/dev/null 2>&1 || true
  rmdir "$mount_root" >/dev/null 2>&1 || true
}
trap cleanup EXIT

hdiutil attach -readonly -nobrowse -mountpoint "$mount_root" "$output_path" >/dev/null
codesign --verify --deep --strict --verbose=2 "$mount_root/Kitter.app"
hdiutil detach "$mount_root" >/dev/null
rmdir "$mount_root"
trap - EXIT
