# Install the standalone Kitter CLI

Use this guide only when the Kitter Skill cannot find a bundled executable or a `kitter` command on `PATH`.

Official standalone CLI packages are published for macOS arm64, Linux x86_64, and Windows x86_64. Detect the current platform and architecture first. If there is no exact matching asset, stop and link the user to <https://github.com/what1f/kitter/releases/latest>; never substitute a binary built for another target.

Always explain where the CLI will be downloaded and installed, then ask for approval before downloading it. Do not install GitHub CLI, another package manager, or modify a shell profile automatically.

## macOS

The supported release target is Apple Silicon (`arm64`). Install the CLI at `~/.local/bin/kitter`:

```bash
test "$(uname -s)" = "Darwin"
test "$(uname -m)" = "arm64"
kitter_cli_tmp="$(mktemp -d)"

gh release download \
  --repo what1f/kitter \
  --pattern "Kitter-*-cli-macos-arm64.tar.gz" \
  --output "${kitter_cli_tmp}/kitter.tar.gz"

mkdir -p "${kitter_cli_tmp}/release" "$HOME/.local/bin"
tar -xzf "${kitter_cli_tmp}/kitter.tar.gz" -C "${kitter_cli_tmp}/release"
install -m 755 "${kitter_cli_tmp}/release/kitter" "$HOME/.local/bin/kitter"
"$HOME/.local/bin/kitter" --help
```

## Linux

The supported release target is x86_64 Linux with glibc:

```bash
test "$(uname -s)" = "Linux"
test "$(uname -m)" = "x86_64"
kitter_cli_tmp="$(mktemp -d)"

gh release download \
  --repo what1f/kitter \
  --pattern "Kitter-*-cli-linux-x86_64.tar.gz" \
  --output "${kitter_cli_tmp}/kitter.tar.gz"

mkdir -p "${kitter_cli_tmp}/release" "$HOME/.local/bin"
tar -xzf "${kitter_cli_tmp}/kitter.tar.gz" -C "${kitter_cli_tmp}/release"
install -m 755 "${kitter_cli_tmp}/release/kitter" "$HOME/.local/bin/kitter"
"$HOME/.local/bin/kitter" --help
```

## Windows

The supported release target is Windows x86_64. Install the executable under the current user's local application data directory:

```powershell
if ($env:PROCESSOR_ARCHITECTURE -ne "AMD64") {
    throw "No Kitter CLI release is available for $env:PROCESSOR_ARCHITECTURE"
}

$kitterCliTmp = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid())
$kitterCliInstall = Join-Path $env:LOCALAPPDATA "Kitter\bin"
New-Item -ItemType Directory -Force -Path $kitterCliTmp, $kitterCliInstall | Out-Null

gh release download `
  --repo what1f/kitter `
  --pattern "Kitter-*-cli-windows-x86_64.zip" `
  --output (Join-Path $kitterCliTmp "kitter.zip")

Expand-Archive -Path (Join-Path $kitterCliTmp "kitter.zip") -DestinationPath (Join-Path $kitterCliTmp "release")
Copy-Item (Join-Path $kitterCliTmp "release\kitter.exe") (Join-Path $kitterCliInstall "kitter.exe") -Force
& (Join-Path $kitterCliInstall "kitter.exe") --help
```

Keep using the installed absolute path for the current task. If its directory is not already on `PATH`, tell the user what directory they may add for future shells, but do not change `PATH` yourself.

## Without GitHub CLI

If `gh` is unavailable, direct the user to <https://github.com/what1f/kitter/releases/latest> and ask them to download the exact CLI archive for their platform and architecture. After extraction, place `kitter` or `kitter.exe` in a directory already on `PATH`, or keep using its absolute path. Do not install `gh` automatically and do not fall back to building from source without the user's approval.
