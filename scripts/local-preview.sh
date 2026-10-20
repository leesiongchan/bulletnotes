#!/usr/bin/env bash
# Reproducible ad-hoc signed local build. Every bundle gets a new path, so a live
# preview executable is never overwritten. No HTTP or production signing occurs.
set -euo pipefail
app_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$app_root"
python3 scripts/prepare-slint.py
rustup run 1.99.1 cargo build --bin bulletnotes --locked
app_target="${CARGO_TARGET_DIR:-$app_root/target}"
mkdir -p "$app_target/local-preview"
app_build="$(mktemp -d "$app_target/local-preview/build.XXXXXX")"
app_bundle="$app_build/Bulletnotes Local.app"
mkdir -p "$app_bundle/Contents/MacOS"
cp "$app_target/debug/bulletnotes" "$app_bundle/Contents/MacOS/bulletnotes"
cat > "$app_bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>lol.harflix.bulletnotes.local</string>
<key>CFBundleExecutable</key><string>bulletnotes</string>
<key>CFBundleName</key><string>Bulletnotes Local</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleVersion</key><string>1</string>
<key>LSMinimumSystemVersion</key><string>14.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSPrincipalClass</key><string>BulletnotesApplication</string>
</dict></plist>
PLIST
codesign --force --sign - --timestamp=none "$app_bundle"
codesign --verify --strict "$app_bundle"
printf '%s\n' "$app_bundle"
if [[ "${1:-}" != "--build-only" ]]; then
    open "$app_bundle"
fi
