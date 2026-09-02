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
