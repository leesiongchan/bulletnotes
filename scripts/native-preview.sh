#!/usr/bin/env bash
# Ad-hoc signed validation bundle only; not a distribution/release package.
set -euo pipefail
probe_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$probe_root"
rustup run 1.99.1 cargo build --features native-validation --example native_gate --locked
probe_target="${CARGO_TARGET_DIR:-$probe_root/target}"
probe_app="$probe_target/native-preview/Bulletnotes Native Preview.app"
mkdir -p "$probe_app/Contents/MacOS"
cp "$probe_target/debug/examples/native_gate" "$probe_app/Contents/MacOS/bulletnotes-native-preview"
cat > "$probe_app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>lol.harflix.bulletnotes.native-preview</string>
<key>CFBundleExecutable</key><string>bulletnotes-native-preview</string>
<key>CFBundleName</key><string>Bulletnotes Native Preview</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.0.1</string>
<key>CFBundleVersion</key><string>1</string>
<key>LSMinimumSystemVersion</key><string>14.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSPrincipalClass</key><string>NSApplication</string>
</dict></plist>
PLIST
codesign --force --sign - --timestamp=none "$probe_app"
codesign --verify --strict "$probe_app"
printf '%s\n' "$probe_app"
if [[ "${1:-}" != "--build-only" ]]; then
    open "$probe_app"
fi
