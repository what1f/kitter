# Install the standalone Kitter CLI

Use this guide only when the Kitter Skill cannot find a bundled executable or a `kitter` command on `PATH`.

## Availability

Official binary releases currently target macOS. Windows and Linux builds are coming soon. On an unsupported platform, link the user to <https://github.com/what1f/kitter/releases/latest> and stop; do not download a macOS binary or silently fall back to building from source.

## macOS

Explain that the CLI will be downloaded from the latest official Kitter GitHub Release and installed at `~/.local/bin/kitter`. Ask for approval before running the commands.

If GitHub CLI is available, use a fresh temporary directory and select the asset matching the current architecture:

```bash
kitter_cli_tmp="$(mktemp -d)"
kitter_cli_arch="$(uname -m)"

gh release download \
  --repo what1f/kitter \
  --pattern "Kitter-*-macos-${kitter_cli_arch}.zip" \
  --output "${kitter_cli_tmp}/kitter.zip"

unzip -q "${kitter_cli_tmp}/kitter.zip" -d "${kitter_cli_tmp}/release"
mkdir -p "$HOME/.local/bin"
install -m 755 "${kitter_cli_tmp}/release/kitter" "$HOME/.local/bin/kitter"
"$HOME/.local/bin/kitter" --help
```

If the matching asset does not exist, stop and report the architecture and missing pattern. Do not substitute another architecture.

If `gh` is unavailable, do not install it automatically. Direct the user to <https://github.com/what1f/kitter/releases/latest>, ask them to download the macOS archive matching `uname -m`, and place the archive's root-level `kitter` binary at `~/.local/bin/kitter` or another directory already on `PATH`.

Do not edit a shell profile automatically. If `~/.local/bin` is not on `PATH`, keep using the absolute path for the current task and tell the user what needs to be added for future shells.
