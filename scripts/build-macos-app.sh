#!/usr/bin/env bash
# Build Draft.app from the release binary.
#
#   bash scripts/build-macos-app.sh
#   CODESIGN_IDENTITY="Developer ID Application: ..." bash scripts/build-macos-app.sh
#   MAKE_DMG=1 bash scripts/build-macos-app.sh
#
# On a plain CI runner there is no Developer ID certificate, so the default is
# an ad-hoc signature. That proves the bundle structure and lets TCC prompts
# appear on the build machine, but it is not distributable: Gatekeeper still
# blocks a downloaded ad-hoc build. See docs/MACOS.md stage 8.

set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "build-macos-app.sh must run on macOS" >&2
    exit 1
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

version="${VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)}"
if [[ -z "$version" ]]; then
    echo "could not read the workspace version from Cargo.toml" >&2
    exit 1
fi

echo "building Draft $version for macOS"
cargo build --release -p draft-desktop

app="$root/dist/Draft.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

cp "$root/target/release/draft" "$app/Contents/MacOS/draft"
chmod +x "$app/Contents/MacOS/draft"

sed "s/__VERSION__/$version/g" \
    "$root/packaging/macos/Info.plist" > "$app/Contents/Info.plist"

# Licenses live in Resources, not MacOS: codesign seals Resources, while extra
# files under Contents/MacOS make signature verification fail.
mkdir -p "$app/Contents/Resources/licenses"
cp "$root/LICENSE" "$app/Contents/Resources/licenses/LICENSE.txt"
cp "$root/NOTICE" "$app/Contents/Resources/licenses/NOTICE.txt"
cp "$root/CREDITS.md" "$app/Contents/Resources/licenses/CREDITS.md"
cp "$root/THIRD_PARTY_NOTICES.md" "$app/Contents/Resources/licenses/THIRD_PARTY_NOTICES.md"
cp "$root/assets/fonts/Inter-OFL.txt" "$app/Contents/Resources/licenses/Inter-OFL.txt"
cp "$root/assets/fonts/JetBrainsMono-OFL.txt" \
    "$app/Contents/Resources/licenses/JetBrainsMono-OFL.txt"

# A DMG is the only installer on macOS, so a release build carries the F16
# artifact. `main.rs` reads it from Resources/models when the data folder has no
# model, which keeps the first dictation offline.
if [[ -n "${MODEL_PATH:-}" ]]; then
    if [[ ! -f "$MODEL_PATH" ]]; then
        echo "MODEL_PATH does not exist: $MODEL_PATH" >&2
        exit 1
    fi
    mkdir -p "$app/Contents/Resources/models"
    cp "$MODEL_PATH" "$app/Contents/Resources/models/whisper-large-v3-turbo-F16.gguf"
    echo "bundled model: $MODEL_PATH"
fi

# The repository ships one 256 px source icon. Render the full iconset from it
# so iconutil always gets every slot; the 512/1024 entries are upscaled and can
# be replaced when a native 1024 asset is tracked.
iconset="$root/dist/Draft.iconset"
rm -rf "$iconset"
mkdir -p "$iconset"
source_icon="$root/assets/draft-256.png"
render_icon() {
    sips -z "$2" "$2" "$source_icon" --out "$iconset/$1" >/dev/null
}
render_icon icon_16x16.png 16
render_icon icon_16x16@2x.png 32
render_icon icon_32x32.png 32
render_icon icon_32x32@2x.png 64
render_icon icon_128x128.png 128
render_icon icon_128x128@2x.png 256
render_icon icon_256x256.png 256
render_icon icon_256x256@2x.png 512
render_icon icon_512x512.png 512
render_icon icon_512x512@2x.png 1024
iconutil -c icns "$iconset" -o "$app/Contents/Resources/Draft.icns"
rm -rf "$iconset"

plutil -lint "$app/Contents/Info.plist" >/dev/null

identity="${CODESIGN_IDENTITY:--}"
echo "signing with: $identity"
codesign --force --sign "$identity" "$app"
codesign --verify --strict --verbose=2 "$app"

if [[ "${MAKE_DMG:-0}" == "1" ]]; then
    arch="$(uname -m)"
    dmg="$root/dist/Draft-$version-macos-$arch.dmg"
    stage="$root/dist/dmg-stage"
    rm -rf "$stage" "$dmg"
    mkdir -p "$stage"
    # ditto preserves the signature and extended attributes better than cp.
    ditto "$app" "$stage/Draft.app"
    ln -s /Applications "$stage/Applications"
    hdiutil create -volname Draft -srcfolder "$stage" -ov -format UDZO "$dmg" >/dev/null
    rm -rf "$stage"
    echo "built $dmg"
fi

echo "built $app"
