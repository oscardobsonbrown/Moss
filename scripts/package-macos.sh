#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
profile="${1:-release}"
version="${MOSS_VERSION:-0.1.0}"
if ! printf '%s\n' "$version" | grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$'; then
    printf '%s\n' "MOSS_VERSION must be a stable semantic version." >&2
    exit 1
fi
if [ "$profile" = debug ]; then
    cargo build -p moss -p moss-mcp --jobs 2
else
    cargo build --profile "$profile" -p moss -p moss-mcp --jobs 2
fi
app="dist/Moss.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "target/$profile/moss" "$app/Contents/MacOS/Moss"
cp "target/$profile/moss-mcp" "$app/Contents/MacOS/moss-mcp"
if [ "${MOSS_RELEASE_BUILD:-}" = 1 ] && [ -n "${MOSS_UPDATE_PUBLIC_KEY:-}" ]; then
    xcrun swiftc -O scripts/moss-update-helper.swift -o "$app/Contents/MacOS/moss-update-helper"
else
    rm -f "$app/Contents/MacOS/moss-update-helper"
fi
cp crates/moss-app/assets/Moss.icns "$app/Contents/Resources/Moss.icns"
cp crates/moss-app/assets/fonts/OFL.txt "$app/Contents/Resources/Geist-OFL.txt"
cp crates/moss-app/assets/fonts/LICENSE.txt "$app/Contents/Resources/Geist-LICENSE.txt"
cp crates/moss-app/assets/phosphor/LICENSE "$app/Contents/Resources/Phosphor-MIT.txt"
cp crates/moss-app/assets/emoji/LICENSE-UNICODE.txt "$app/Contents/Resources/Unicode-LICENSE.txt"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>Moss</string>
<key>CFBundleIdentifier</key><string>app.moss.desktop</string>
<key>CFBundleName</key><string>Moss</string>
<key>CFBundleDisplayName</key><string>Moss</string>
<key>CFBundleIconFile</key><string>Moss</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleVersion</key><string>1</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSSupportsAutomaticTermination</key><false/>
</dict></plist>
PLIST
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" "$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $version" "$app/Contents/Info.plist"
codesign --force --sign - "$app"
printf '%s\n' "Built $app (local, ad hoc signature)."
if [ "${CI:-}" != true ] && [ "${GITHUB_ACTIONS:-}" != true ]; then
    scripts/install-macos.sh "$app" /Applications/Moss.app
fi
