#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

APP_NAME="Sennheiser Control.app"
BUNDLE_DIR="target/release/bundle/${APP_NAME}"
CONTENTS="${BUNDLE_DIR}/Contents"
MACOS_DIR="${CONTENTS}/MacOS"
RESOURCES_DIR="${CONTENTS}/Resources"
ICONSET_DIR="target/release/AppIcon.iconset"

echo "==> Building sennheiser-control (release)..."
cargo build --release --manifest-path Cargo.toml

echo "==> Rendering macOS iconset..."
mkdir -p "${ICONSET_DIR}"
./target/release/sennheiser-control --render-icons "${ICONSET_DIR}"

echo "==> Creating ${APP_NAME} bundle..."
rm -rf "${BUNDLE_DIR}"
mkdir -p "${MACOS_DIR}" "${RESOURCES_DIR}"

cp target/release/sennheiser-control "${MACOS_DIR}/sennheiser-control"
cp Info.plist "${CONTENTS}/Info.plist"

if command -v iconutil >/dev/null 2>&1; then
    iconutil -c icns "${ICONSET_DIR}" -o "${RESOURCES_DIR}/AppIcon.icns"
fi

# Ad-hoc sign so macOS TCC remembers Bluetooth permission across launches
if command -v codesign >/dev/null 2>&1; then
    codesign --force --deep --sign - "${BUNDLE_DIR}"
fi

echo "==> Built: macos/${BUNDLE_DIR}"
echo "    Run with: open \"macos/${BUNDLE_DIR}\""

