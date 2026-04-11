#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_TRIPLE="${TARGET_TRIPLE:-aarch64-apple-darwin}"
OUTPUT_DIR="${OUTPUT_DIR:-$ROOT_DIR/dist}"
BUILD_ARGS="${TAURI_BUILD_ARGS:-}"

usage() {
    cat <<'EOF'
Usage: ./scripts/release.sh <dmg-name>

Build an Apple Silicon OpenDuck DMG and copy it to dist/<dmg-name>.dmg

Environment variables:
  OUTPUT_DIR=/path              Override the output directory.
  TARGET_TRIPLE=triple          Override the Tauri target triple.
  TAURI_BUILD_ARGS="..."        Extra args appended to tauri build.

Examples:
  ./scripts/release.sh openduck-beta-v0
  TAURI_BUILD_ARGS="--no-sign" ./scripts/release.sh openduck-beta-v1
EOF
}

die() {
    echo "Error: $*" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || die "Missing required command: $1"
}

normalize_name() {
    local raw_name="$1"
    local normalized="${raw_name%.dmg}"

    [ -n "$normalized" ] || die "DMG name cannot be empty."
    [[ "$normalized" != */* ]] || die "DMG name must be a file name, not a path."
    [[ "$normalized" != "." && "$normalized" != ".." ]] || die "Invalid DMG name: $normalized"

    printf '%s\n' "$normalized"
}

find_latest_dmg() {
    local bundle_dir="$1"
    local candidates=()
    local latest=""
    local candidate

    shopt -s nullglob
    candidates=("$bundle_dir"/*.dmg)
    shopt -u nullglob

    [ "${#candidates[@]}" -gt 0 ] || return 1

    latest="${candidates[0]}"
    for candidate in "${candidates[@]}"; do
        if [ "$candidate" -nt "$latest" ]; then
            latest="$candidate"
        fi
    done

    printf '%s\n' "$latest"
}

main() {
    local requested_name="${1:-}"
    local extra_arg="${2:-}"
    local dmg_name=""
    local bundle_dir=""
    local source_dmg=""
    local output_dmg=""

    [ -n "$requested_name" ] || {
        usage
        exit 1
    }

    [ -z "$extra_arg" ] || {
        usage
        die "Unexpected argument: $extra_arg"
    }

    dmg_name="$(normalize_name "$requested_name")"
    bundle_dir="$ROOT_DIR/src-tauri/target/$TARGET_TRIPLE/release/bundle/dmg"
    output_dmg="$OUTPUT_DIR/$dmg_name.dmg"

    [ "$(uname -s)" = "Darwin" ] || die "release.sh must be run on macOS."
    [ "$(uname -m)" = "arm64" ] || die "release.sh is configured for Apple Silicon hosts."

    require_command npm
    require_command cargo

    if [ ! -d "$ROOT_DIR/node_modules" ]; then
        echo "Installing npm dependencies..."
        (cd "$ROOT_DIR" && npm install)
    fi

    mkdir -p "$OUTPUT_DIR"

    echo "Building OpenDuck DMG for $TARGET_TRIPLE..."
    if [ -n "$BUILD_ARGS" ]; then
        # shellcheck disable=SC2206
        local build_args_array=($BUILD_ARGS)
        (cd "$ROOT_DIR" && npm run tauri build -- --target "$TARGET_TRIPLE" --bundles dmg "${build_args_array[@]}")
    else
        (cd "$ROOT_DIR" && npm run tauri build -- --target "$TARGET_TRIPLE" --bundles dmg)
    fi

    source_dmg="$(find_latest_dmg "$bundle_dir")" || die "No DMG was produced in $bundle_dir"

    cp -f "$source_dmg" "$output_dmg"

    echo "Release DMG ready:"
    echo "$output_dmg"
}

main "$@"
