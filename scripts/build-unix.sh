#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
version="$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml","rb"))["package"]["version"])')"
os="$(uname -s)"
arch="$(uname -m)"
cargo build --release --locked
target="${CARGO_TARGET_DIR:-target}/release/tickreel-rust"
mkdir -p dist
if [[ "$os" == Darwin ]]; then
  root="dist/Tickreel-macos-$arch"
  app="$root/Tickreel.app"
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/licenses"
  cp "$target" "$app/Contents/MacOS/tickreel-rust"
  cp node_modules/ffmpeg-static/ffmpeg "$app/Contents/MacOS/ffmpeg"
  cp node_modules/ffmpeg-static/ffmpeg.LICENSE node_modules/ffmpeg-static/ffmpeg.README "$app/Contents/Resources/licenses/"
  python3 scripts/collect-licenses.py "$root/licenses"
  cp README.md "$root/README.md"
  cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Tickreel</string>
<key>CFBundleDisplayName</key><string>Tickreel</string>
<key>CFBundleIdentifier</key><string>com.appipinopi.tickreel</string>
<key>CFBundleExecutable</key><string>tickreel-rust</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
<key>LSMinimumSystemVersion</key><string>12.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSMicrophoneUsageDescription</key><string>制作のタイムラプスに、自分の声のアフレコを録音して追加します。</string>
</dict></plist>
EOF
  chmod +x "$app/Contents/MacOS/"*
  codesign --force --deep --sign - "$app"
  ditto -c -k --sequesterRsrc --keepParent "$root" "dist/Tickreel-$version-macos-$arch.zip"
else
  root="dist/Tickreel-linux-$arch"
  mkdir -p "$root/licenses"
  cp "$target" "$root/tickreel"
  cp node_modules/ffmpeg-static/ffmpeg "$root/ffmpeg"
  cp node_modules/ffmpeg-static/ffmpeg.LICENSE node_modules/ffmpeg-static/ffmpeg.README "$root/licenses/"
  python3 scripts/collect-licenses.py "$root/licenses"
  cp README.md "$root/README.md"
  chmod +x "$root/tickreel" "$root/ffmpeg"
  tar -czf "dist/Tickreel-$version-linux-$arch.tar.gz" -C dist "Tickreel-linux-$arch"
fi
