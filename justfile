set shell := ["zsh", "-cu"]

root := justfile_directory()
release_dir := root + "/target/release"
app_dir := release_dir + "/Kitter.app"
icon_source := root + "/assets/app-icon.png"
version := `sed -nE 's/^version = "([^"]+)"/\1/p' Cargo.toml | head -n 1`
architecture := `uname -m`
archive := release_dir + "/Kitter-" + version + "-macos-" + architecture + ".zip"

default:
    @just --list

check:
    cargo test --locked

build:
    cargo build --release --locked --features desktop --bin kitter-desktop --bin kitter

app: build
    rm -rf "{{app_dir}}"
    rm -rf "{{release_dir}}/logo.iconset"
    mkdir -p "{{app_dir}}/Contents/MacOS" "{{app_dir}}/Contents/Resources/kitter-skill/bin" "{{release_dir}}/logo.iconset"
    sips -z 16 16 "{{icon_source}}" --out "{{release_dir}}/logo.iconset/icon_16x16.png"
    sips -z 32 32 "{{icon_source}}" --out "{{release_dir}}/logo.iconset/icon_16x16@2x.png"
    sips -z 32 32 "{{icon_source}}" --out "{{release_dir}}/logo.iconset/icon_32x32.png"
    sips -z 64 64 "{{icon_source}}" --out "{{release_dir}}/logo.iconset/icon_32x32@2x.png"
    sips -z 128 128 "{{icon_source}}" --out "{{release_dir}}/logo.iconset/icon_128x128.png"
    sips -z 256 256 "{{icon_source}}" --out "{{release_dir}}/logo.iconset/icon_128x128@2x.png"
    sips -z 256 256 "{{icon_source}}" --out "{{release_dir}}/logo.iconset/icon_256x256.png"
    sips -z 512 512 "{{icon_source}}" --out "{{release_dir}}/logo.iconset/icon_256x256@2x.png"
    sips -z 512 512 "{{icon_source}}" --out "{{release_dir}}/logo.iconset/icon_512x512.png"
    sips -z 1024 1024 "{{icon_source}}" --out "{{release_dir}}/logo.iconset/icon_512x512@2x.png"
    iconutil -c icns "{{release_dir}}/logo.iconset" -o "{{app_dir}}/Contents/Resources/logo.icns"
    cp "{{release_dir}}/kitter-desktop" "{{app_dir}}/Contents/MacOS/Kitter"
    cp "{{release_dir}}/kitter" "{{app_dir}}/Contents/Resources/kitter-skill/bin/kitter"
    cp "{{root}}/resources/Info.plist" "{{app_dir}}/Contents/Info.plist"
    cp "{{root}}/LICENSE" "{{root}}/THIRD_PARTY_LICENSES.md" "{{app_dir}}/Contents/Resources/"
    plutil -replace CFBundleShortVersionString -string "{{version}}" "{{app_dir}}/Contents/Info.plist"
    plutil -replace CFBundleVersion -string "{{version}}" "{{app_dir}}/Contents/Info.plist"
    chmod +x "{{app_dir}}/Contents/MacOS/Kitter" "{{app_dir}}/Contents/Resources/kitter-skill/bin/kitter"
    xattr -cr "{{app_dir}}"
    codesign --deep --force --sign - "{{app_dir}}"
    codesign --verify --deep --strict "{{app_dir}}"
    @echo "App ready: {{app_dir}}"

package: app
    rm -f "{{archive}}"
    rm -rf "{{release_dir}}/assets/readme"
    mkdir -p "{{release_dir}}/assets"
    cp -R "{{root}}/assets/readme" "{{release_dir}}/assets/"
    cp "{{root}}/README.md" "{{root}}/README.zh-CN.md" "{{root}}/LICENSE" "{{root}}/THIRD_PARTY_LICENSES.md" "{{release_dir}}/"
    (cd "{{release_dir}}" && zip -qry "{{archive}}" Kitter.app README.md README.zh-CN.md assets/readme LICENSE THIRD_PARTY_LICENSES.md)
    unzip -tq "{{archive}}"
    @file "{{release_dir}}/kitter" "{{release_dir}}/kitter-desktop"
    @shasum -a 256 "{{archive}}"
    @echo "Package ready: {{archive}}"

run: app
    open "{{app_dir}}"

release: check package
    @echo "Release ready: {{archive}}"
