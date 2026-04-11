#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_TRIPLE="${TARGET_TRIPLE:-aarch64-apple-darwin}"
OUTPUT_DIR="${OUTPUT_DIR:-$ROOT_DIR/dist}"
BUILD_ARGS="${TAURI_BUILD_ARGS:-}"
VERSION_OVERRIDE="${VERSION:-}"
VERSION_LABEL_OVERRIDE="${VERSION_LABEL:-}"
BUILD_CHANNEL_OVERRIDE="${BUILD_CHANNEL:-}"
BUILD_NUMBER_OVERRIDE="${BUILD_NUMBER:-}"
TEMP_TAURI_CONFIG=""

usage() {
    cat <<'EOF'
Usage: ./scripts/release.sh <dmg-name> [app-version]

Build an Apple Silicon OpenDuck DMG and copy it to dist/<dmg-name>.dmg

Environment variables:
  OUTPUT_DIR=/path              Override the output directory.
  TARGET_TRIPLE=triple          Override the Tauri target triple.
  TAURI_BUILD_ARGS="..."        Extra args appended to tauri build.
  VERSION=x.y.z                 Override the app version for this build only.
  VERSION_LABEL="Beta"          Optional release label shown in the About dialog.
  BUILD_CHANNEL=beta            Optional build channel used in the About dialog/build id.
  BUILD_NUMBER=221              Optional build number used in the About dialog/build id.

Examples:
  ./scripts/release.sh openduck-beta-v0
  ./scripts/release.sh openduck-1.2.3 1.2.3
  VERSION=1.2.3 ./scripts/release.sh openduck-1.2.3
  VERSION=1.2.3 VERSION_LABEL="Beta" BUILD_CHANNEL=beta BUILD_NUMBER=221 ./scripts/release.sh openduck-beta-221
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

normalize_version() {
    local version="$1"

    [ -n "$version" ] || die "App version cannot be empty."
    [[ "$version" != *[[:space:]]* ]] || die "App version cannot contain whitespace."
    [[ "$version" =~ ^[0-9A-Za-z.+-]+$ ]] || die "App version may only contain letters, digits, dots, plus signs, and hyphens."

    printf '%s\n' "$version"
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
    local requested_version="${2:-}"
    local extra_arg="${3:-}"
    local dmg_name=""
    local app_version=""
    local bundle_dir=""
    local source_dmg=""
    local output_dmg=""
    local config_args=()
    local build_env=()

    [ -n "$requested_name" ] || {
        usage
        exit 1
    }

    [ -z "$VERSION_OVERRIDE" ] || [ -z "$requested_version" ] || {
        usage
        die "Specify the app version either as the second argument or via VERSION, not both."
    }

    [ -z "$extra_arg" ] || {
        usage
        die "Unexpected argument: $extra_arg"
    }

    dmg_name="$(normalize_name "$requested_name")"
    if [ -n "$VERSION_OVERRIDE" ] || [ -n "$requested_version" ]; then
        app_version="$(normalize_version "${VERSION_OVERRIDE:-$requested_version}")"
    fi
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
    if [ -n "$app_version" ]; then
        TEMP_TAURI_CONFIG="$(mktemp "${TMPDIR:-/tmp}/openduck-tauri-version.XXXXXX.json")"
        trap 'rm -f "${TEMP_TAURI_CONFIG:-}"' EXIT
        printf '{\n  "version": "%s"\n}\n' "$app_version" >"$TEMP_TAURI_CONFIG"
        config_args=(--config "$TEMP_TAURI_CONFIG")
        echo "Overriding app version for this build: $app_version"
        build_env+=("OPEN_DUCK_BUILD_VERSION=$app_version")
    fi

    if [ -n "$VERSION_LABEL_OVERRIDE" ]; then
        echo "Using About label for this build: $VERSION_LABEL_OVERRIDE"
        build_env+=("OPEN_DUCK_BUILD_LABEL=$VERSION_LABEL_OVERRIDE")
    fi

    if [ -n "$BUILD_CHANNEL_OVERRIDE" ]; then
        echo "Using build channel for this build: $BUILD_CHANNEL_OVERRIDE"
        build_env+=("OPEN_DUCK_BUILD_CHANNEL=$BUILD_CHANNEL_OVERRIDE")
    fi

    if [ -n "$BUILD_NUMBER_OVERRIDE" ]; then
        echo "Using build number for this build: $BUILD_NUMBER_OVERRIDE"
        build_env+=("OPEN_DUCK_BUILD_NUMBER=$BUILD_NUMBER_OVERRIDE")
    fi

    if [ -n "$BUILD_ARGS" ]; then
        # shellcheck disable=SC2206
        local build_args_array=($BUILD_ARGS)
        (cd "$ROOT_DIR" && env "${build_env[@]}" npm run tauri build -- --target "$TARGET_TRIPLE" --bundles dmg "${config_args[@]}" "${build_args_array[@]}")
    else
        (cd "$ROOT_DIR" && env "${build_env[@]}" npm run tauri build -- --target "$TARGET_TRIPLE" --bundles dmg "${config_args[@]}")
    fi

    source_dmg="$(find_latest_dmg "$bundle_dir")" || die "No DMG was produced in $bundle_dir"

    cp -f "$source_dmg" "$output_dmg"

    echo "Release DMG ready:"
    echo "$output_dmg"
}

main "$@"
