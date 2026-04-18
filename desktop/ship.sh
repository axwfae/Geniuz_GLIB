#!/bin/bash
# ship.sh — build, sign, notarize, staple, package Geniuz.dmg
# Produces: build/Geniuz.dmg — ready to upload to GitHub Releases.
# Assumes: Developer ID Application cert in Keychain, AC_PASSWORD notary profile.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DESKTOP="$ROOT/desktop"
CLI_ARM64="$ROOT/target/aarch64-apple-darwin/release/geniuz"
ARCHIVE="$DESKTOP/build/Geniuz.xcarchive"
EXPORT_DIR="$DESKTOP/build/export"
APP="$EXPORT_DIR/Geniuz.app"
DMG="$DESKTOP/build/Geniuz.dmg"
DMG_STAGING="$DESKTOP/build/dmg-staging"

IDENTITY="Developer ID Application: Managed Ventures LLC (NT5SU826F4)"
TEAM_ID="NT5SU826F4"
NOTARY_PROFILE="AC_PASSWORD"

echo "==> Step 1/8: Build arm64 Rust CLI"
cd "$ROOT"
cargo build --release --target aarch64-apple-darwin

echo "==> Step 2/8: xcodebuild archive (Release, universal Swift)"
cd "$DESKTOP"
rm -rf "$ARCHIVE" "$EXPORT_DIR"
xcodebuild archive \
    -project Geniuz.xcodeproj \
    -scheme Geniuz \
    -configuration Release \
    -archivePath "$ARCHIVE" \
    -destination 'generic/platform=macOS' \
    SKIP_INSTALL=NO \
    | tail -5

echo "==> Step 3/8: Inject Rust CLI into archived .app's Resources/"
cp "$CLI_ARM64" "$ARCHIVE/Products/Applications/Geniuz.app/Contents/Resources/geniuz"
chmod +x "$ARCHIVE/Products/Applications/Geniuz.app/Contents/Resources/geniuz"

echo "==> Step 4/8: Re-sign the .app (CLI injection invalidated outer signature)"
codesign --force --options runtime --timestamp \
    --sign "$IDENTITY" \
    "$ARCHIVE/Products/Applications/Geniuz.app/Contents/Resources/geniuz"
codesign --force --options runtime --timestamp \
    --sign "$IDENTITY" \
    --entitlements "$DESKTOP/Geniuz/Geniuz.entitlements" \
    "$ARCHIVE/Products/Applications/Geniuz.app"

echo "==> Step 5/8: Export signed .app to export dir"
mkdir -p "$EXPORT_DIR"
cp -R "$ARCHIVE/Products/Applications/Geniuz.app" "$APP"

echo "==> Step 6/8: Notarize .app (submit → wait → staple)"
APP_ZIP="$DESKTOP/build/Geniuz.app.zip"
rm -f "$APP_ZIP"
ditto -c -k --keepParent "$APP" "$APP_ZIP"
xcrun notarytool submit "$APP_ZIP" --keychain-profile "$NOTARY_PROFILE" --wait
xcrun stapler staple "$APP"

echo "==> Step 7/8: Build DMG"
rm -rf "$DMG_STAGING" "$DMG"
mkdir -p "$DMG_STAGING"
cp -R "$APP" "$DMG_STAGING/"
ln -s /Applications "$DMG_STAGING/Applications"
hdiutil create -volname "Geniuz" \
    -srcfolder "$DMG_STAGING" \
    -ov -format UDZO \
    "$DMG"

echo "==> Step 8/8: Sign + notarize DMG"
codesign --force --timestamp --sign "$IDENTITY" "$DMG"
xcrun notarytool submit "$DMG" --keychain-profile "$NOTARY_PROFILE" --wait
xcrun stapler staple "$DMG"

echo ""
echo "✅ Ship complete: $DMG"
ls -lh "$DMG"
