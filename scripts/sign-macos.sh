#!/usr/bin/env bash
# Sign (and, when credentials are set, notarize) macOS release artifacts.
#
#   sign-macos.sh app <Kitter.app> <kitter-cli>   sign the app and CLI, notarize both, staple the app
#   sign-macos.sh dmg <Kitter.dmg>                sign, notarize and staple the installer
#
# Environment:
#   KITTER_SIGN_IDENTITY   "Developer ID Application: …" identity. Unset means ad-hoc signing
#                          for local builds; notarization is skipped.
#   KITTER_NOTARY_PROFILE  notarytool keychain profile (xcrun notarytool store-credentials), or
#   APPLE_API_KEY_PATH, APPLE_API_KEY_ID, APPLE_API_ISSUER_ID  App Store Connect API key.

set -euo pipefail

identity="${KITTER_SIGN_IDENTITY:-}"

sign() {
  if [ -z "$identity" ]; then
    codesign --force --sign - "$@"
  else
    codesign --force --options runtime --timestamp --sign "$identity" "$@"
  fi
}

can_notarize() {
  [ -n "$identity" ] && { [ -n "${KITTER_NOTARY_PROFILE:-}" ] || [ -n "${APPLE_API_KEY_PATH:-}" ]; }
}

notarize() {
  local credentials
  if [ -n "${KITTER_NOTARY_PROFILE:-}" ]; then
    credentials=(--keychain-profile "$KITTER_NOTARY_PROFILE")
  else
    credentials=(--key "$APPLE_API_KEY_PATH" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER_ID")
  fi
  local id status=""
  # Poll instead of `--wait`: Apple's gateway sometimes times out mid-wait.
  for attempt in 1 2 3 4 5; do
    id="$(xcrun notarytool submit "$1" "${credentials[@]}" --output-format json \
      | sed -nE 's/.*"id" *: *"([^"]+)".*/\1/p' | head -n 1)" || true
    [ -n "$id" ] && break
    echo "Notarization upload failed (attempt $attempt), retrying" >&2
    sleep 30
  done
  if [ -z "$id" ]; then
    echo "Could not submit $1 for notarization" >&2
    exit 1
  fi
  echo "Submitted $1 for notarization: $id"
  while :; do
    status="$(xcrun notarytool info "$id" "${credentials[@]}" --output-format json 2>/dev/null \
      | sed -nE 's/.*"status" *: *"([^"]+)".*/\1/p' | head -n 1)" || true
    case "$status" in
      Accepted) break ;;
      Invalid|Rejected)
        xcrun notarytool log "$id" "${credentials[@]}" || true
        echo "Notarization $status for $1" >&2
        exit 1
        ;;
    esac
    sleep 30
  done
  echo "Notarization accepted for $1"
}

case "${1:-}" in
  app)
    app="$2"
    cli="$3"
    sign --identifier dev.kitter.cli "$cli"
    sign "$app"
    codesign --verify --strict --verbose=2 "$cli"
    codesign --verify --deep --strict --verbose=2 "$app"
    if can_notarize; then
      work="$(mktemp -d "${TMPDIR:-/tmp}/kitter-notary.XXXXXX")"
      trap 'rm -rf "$work"' EXIT
      ditto "$app" "$work/payload/Kitter.app"
      cp "$cli" "$work/payload/kitter"
      ditto -c -k "$work/payload" "$work/Kitter.zip"
      notarize "$work/Kitter.zip"
      xcrun stapler staple "$app"
      spctl --assess --type execute --verbose=2 "$app"
    fi
    ;;
  dmg)
    dmg="$2"
    if [ -n "$identity" ]; then
      codesign --force --timestamp --sign "$identity" "$dmg"
      codesign --verify --strict --verbose=2 "$dmg"
    fi
    if can_notarize; then
      notarize "$dmg"
      xcrun stapler staple "$dmg"
      spctl --assess --type open --context context:primary-signature --verbose=2 "$dmg"
    fi
    ;;
  *)
    echo "Usage: $0 app <Kitter.app> <kitter-cli> | dmg <Kitter.dmg>" >&2
    exit 2
    ;;
esac
