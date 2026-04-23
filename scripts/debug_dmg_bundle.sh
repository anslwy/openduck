#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_TRIPLE="${TARGET_TRIPLE:-aarch64-apple-darwin}"
BUNDLE_DIR="$ROOT_DIR/src-tauri/target/$TARGET_TRIPLE/release/bundle"
DMG_SCRIPT="$BUNDLE_DIR/dmg/bundle_dmg.sh"
ICON_FILE="$BUNDLE_DIR/dmg/icon.icns"
SOURCE_DIR="${SOURCE_DIR:-$BUNDLE_DIR/macos}"
OUTPUT_DIR="${OUTPUT_DIR:-$BUNDLE_DIR/dmg}"
TIMESTAMP="$(date +%Y%m%d-%H%M%S)"
LOG_PATH="${LOG_PATH:-$OUTPUT_DIR/bundle_dmg_debug_$TIMESTAMP.log}"

die() {
  echo "Error: $*" >&2
  exit 1
}

[ -x "$DMG_SCRIPT" ] || die "Generated DMG script not found: $DMG_SCRIPT"
[ -d "$SOURCE_DIR" ] || die "DMG source directory not found: $SOURCE_DIR"
[ -f "$ICON_FILE" ] || die "Generated DMG icon not found: $ICON_FILE"

shopt -s nullglob
apps=("$SOURCE_DIR"/*.app)
shopt -u nullglob

[ "${#apps[@]}" -gt 0 ] || die "No .app bundle found in $SOURCE_DIR. Run a Tauri build first."

APP_PATH="${APP_PATH:-${apps[0]}}"
[ -d "$APP_PATH" ] || die "App bundle not found: $APP_PATH"

APP_NAME="$(basename "$APP_PATH")"
VOLUME_NAME="${DMG_VOLUME_NAME:-${APP_NAME%.app}}"
OUTPUT_DMG="${OUTPUT_DMG:-$OUTPUT_DIR/${VOLUME_NAME}_${TARGET_TRIPLE}_debug.dmg}"

mkdir -p "$OUTPUT_DIR"

if [ -e "$OUTPUT_DMG" ]; then
  die "Output DMG already exists: $OUTPUT_DMG. Set OUTPUT_DMG or remove the file and retry."
fi

echo "Replaying generated DMG bundle command with verbose hdiutil output..."
echo "Target triple: $TARGET_TRIPLE"
echo "App bundle: $APP_PATH"
echo "Source dir: $SOURCE_DIR"
echo "Volume name: $VOLUME_NAME"
echo "Output DMG: $OUTPUT_DMG"
echo "Log file: $LOG_PATH"
echo

cmd=(
  "$DMG_SCRIPT"
  --volname "$VOLUME_NAME"
  --volicon "$ICON_FILE"
  --app-drop-link 380 205
  --icon "$APP_NAME" 124 205
  --hide-extension "$APP_NAME"
  --no-internet-enable
  --hdiutil-verbose
  "$OUTPUT_DMG"
  "$SOURCE_DIR"
)

printf 'Running:\n  '
printf '%q ' "${cmd[@]}"
printf '\n\n'

set +e
"${cmd[@]}" 2>&1 | tee "$LOG_PATH"
status=${PIPESTATUS[0]}
set -e

if [ "$status" -ne 0 ]; then
  echo
  echo "bundle_dmg.sh failed with exit code $status"
  echo "Saved verbose log to: $LOG_PATH"
  echo "Next checks:"
  echo "  - mounted OpenDuck volumes under /Volumes"
  echo "  - stale OpenDuck helper processes using a previous DMG mount"
  echo "  - hdiutil or osascript errors in the saved log"
  exit "$status"
fi

echo
echo "Created debug DMG: $OUTPUT_DMG"
echo "Saved verbose log to: $LOG_PATH"
