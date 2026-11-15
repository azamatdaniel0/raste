#!/bin/bash
# Builds New-Paste.app (macOS) into dist/ and zips it for distribution.
# Usage: scripts/bundle-macos.sh [--install]   (--install also copies it to /Applications)
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release -p new-paste

APP="dist/New-Paste.app"
rm -rf dist
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" dist/icon.iconset

cp target/release/new-paste "$APP/Contents/MacOS/new-paste"
cp -R assets "$APP/Contents/MacOS/assets"

ICON=assets/icons/paste-icon.png
for s in 16 32 128 256; do
  sips -z $s $s "$ICON" --out "dist/icon.iconset/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2))
  sips -z $d $d "$ICON" --out "dist/icon.iconset/icon_${s}x${s}@2x.png" >/dev/null
done
cp "$ICON" dist/icon.iconset/icon_512x512.png
iconutil -c icns dist/icon.iconset -o "$APP/Contents/Resources/AppIcon.icns"
rm -rf dist/icon.iconset

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>New-Paste</string>
  <key>CFBundleDisplayName</key><string>New-Paste</string>
  <key>CFBundleIdentifier</key><string>io.artcraft.new-paste</string>
  <key>CFBundleExecutable</key><string>new-paste</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

codesign --force --deep -s - "$APP"
codesign --verify --deep "$APP"

ARCH="$(uname -m)"
ZIP="dist/New-Paste-macos-${ARCH}.zip"
ditto -c -k --keepParent "$APP" "$ZIP"
echo "Built $APP and $ZIP"

if [[ "${1:-}" == "--install" ]]; then
  pkill -x new-paste 2>/dev/null || true
  rm -rf /Applications/New-Paste.app
  cp -R "$APP" /Applications/New-Paste.app
  echo "Installed to /Applications/New-Paste.app"
fi
