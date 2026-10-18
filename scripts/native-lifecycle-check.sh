#!/usr/bin/env bash
# LaunchServices lifecycle check; generated database and loopback content only.
set -euo pipefail
app_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$app_root"
rustup run 1.99.1 cargo build --example app_lifecycle_review --features native-validation,protocol-validation --locked
app_target="${CARGO_TARGET_DIR:-$app_root/target}"
mkdir -p "$app_target/native-lifecycle-review"
app_build="$(mktemp -d "$app_target/native-lifecycle-review/build.XXXXXX")"
app_bundle="$app_build/Bulletnotes Lifecycle Check.app"
mkdir -p "$app_bundle/Contents/MacOS"
cp "$app_target/debug/examples/app_lifecycle_review" "$app_bundle/Contents/MacOS/lifecycle"
cat > "$app_bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>lol.harflix.bulletnotes.lifecycle</string>
<key>CFBundleExecutable</key><string>lifecycle</string>
<key>CFBundleName</key><string>Bulletnotes Lifecycle Check</string>
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
app_args=()
if [[ "$#" -gt 0 ]]; then app_args=(--args "$@"); fi
open -n -W --env SLINT_BACKEND=winit-femtovg --stdout "$app_build/stdout.log" --stderr "$app_build/stderr.log" "$app_bundle" "${app_args[@]}"
rg 'native_startup_close_and_quit: true' "$app_build/stdout.log" || {
    cat "$app_build/stderr.log"
    exit 1
}
printf 'Lifecycle bundle: %s\n' "$app_bundle"
