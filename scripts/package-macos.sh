#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
profile="${1:-release}"
if [ "$profile" = debug ]; then
    cargo build -p rotion --jobs 2
else
    cargo build --release -p rotion --jobs 2
fi
app="dist/Rotion.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "target/$profile/rotion" "$app/Contents/MacOS/Rotion"
cp crates/rotion-app/assets/fonts/OFL.txt "$app/Contents/Resources/Geist-OFL.txt"
cp crates/rotion-app/assets/fonts/LICENSE.txt "$app/Contents/Resources/Geist-LICENSE.txt"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>Rotion</string>
<key>CFBundleIdentifier</key><string>app.rotion.desktop</string>
<key>CFBundleName</key><string>Rotion</string>
<key>CFBundleDisplayName</key><string>Rotion</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleVersion</key><string>1</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSSupportsAutomaticTermination</key><false/>
</dict></plist>
PLIST
codesign --force --sign - "$app"
printf '%s\n' "Built $app (local, ad hoc signature)."
