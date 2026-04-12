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
GITHUB_REPOSITORY="${GITHUB_REPOSITORY:-anslwy/openduck}"
GITHUB_RELEASE_TAG="${GITHUB_RELEASE_TAG:-${RELEASE_TAG:-}}"
UPDATER_PUBLIC_KEY_OVERRIDE="${OPEN_DUCK_UPDATER_PUBLIC_KEY:-}"
UPDATER_PUBLIC_KEY_PATH="${UPDATER_PUBLIC_KEY_PATH:-$ROOT_DIR/src-tauri/updater-public-key.pem}"
UPDATER_ENDPOINT_OVERRIDE="${OPEN_DUCK_UPDATER_ENDPOINT:-}"
RELEASE_NOTES_OVERRIDE="${RELEASE_NOTES:-}"
RELEASE_NOTES_FILE="${RELEASE_NOTES_FILE:-}"
TEMP_TAURI_CONFIG=""

usage() {
    cat <<'EOF'
Usage: ./scripts/release.sh <dmg-name> [app-version]

Build an Apple Silicon OpenDuck release bundle for GitHub Releases.

Outputs:
  - dist/<dmg-name>.dmg
  - dist/<OpenDuck updater bundle>.app.tar.gz
  - dist/<OpenDuck updater bundle>.app.tar.gz.sig
  - dist/latest.json

Environment variables:
  OUTPUT_DIR=/path              Override the output directory.
  TARGET_TRIPLE=triple          Override the Tauri target triple.
  TAURI_BUILD_ARGS="..."        Extra args appended to tauri build.
  VERSION=x.y.z                 Override the app version for this build only.
  VERSION_LABEL="Beta"          Optional release label shown in the About dialog.
  BUILD_CHANNEL=beta            Optional build channel used in the About dialog/build id.
  BUILD_NUMBER=221              Optional build number used in the About dialog/build id.
  GITHUB_RELEASE_TAG=v1.2.3     Required release tag used in latest.json asset URLs.
  GITHUB_REPOSITORY=owner/repo  GitHub repo used for asset URLs. Defaults to anslwy/openduck.
  OPEN_DUCK_UPDATER_PUBLIC_KEY  Optional public key override embedded into the app.
  UPDATER_PUBLIC_KEY_PATH=path  Public key file path. Defaults to src-tauri/updater-public-key.pem.
  OPEN_DUCK_UPDATER_ENDPOINT=url
                                Optional updater endpoint embedded into the app.
                                Defaults to GitHub latest.json download URL.
  RELEASE_NOTES="..."           Optional release notes written into latest.json.
  RELEASE_NOTES_FILE=path       Optional file whose contents become latest.json notes.
  TAURI_SIGNING_PRIVATE_KEY=... Required Tauri updater signing key content or path.
  TAURI_SIGNING_PRIVATE_KEY_PATH=...
                                Optional signer key path supported by newer Tauri CLIs.

Examples:
  GITHUB_RELEASE_TAG=v1.2.3 ./scripts/release.sh openduck-1.2.3
  GITHUB_RELEASE_TAG=v1.2.3 ./scripts/release.sh openduck-1.2.3 1.2.3
  GITHUB_RELEASE_TAG=v1.2.3 VERSION=1.2.3 ./scripts/release.sh openduck-1.2.3
  GITHUB_RELEASE_TAG=v1.2.3 VERSION=1.2.3 VERSION_LABEL="Beta" BUILD_CHANNEL=beta BUILD_NUMBER=221 ./scripts/release.sh openduck-beta-221
  GITHUB_RELEASE_TAG=v1.2.3 RELEASE_NOTES_FILE=release-notes.md ./scripts/release.sh openduck-1.2.3
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

find_latest_matching() {
    local bundle_dir="$1"
    local pattern="$2"
    local candidates=()
    local latest=""
    local candidate

    shopt -s nullglob
    candidates=("$bundle_dir"/$pattern)
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

read_checked_in_version() {
    node -e "const fs=require('fs'); const pkg=JSON.parse(fs.readFileSync(process.argv[1], 'utf8')); if (!pkg.version) process.exit(1); console.log(pkg.version);" "$ROOT_DIR/package.json"
}

load_updater_public_key() {
    if [ -n "$UPDATER_PUBLIC_KEY_OVERRIDE" ]; then
        printf '%s\n' "$UPDATER_PUBLIC_KEY_OVERRIDE"
        return 0
    fi

    [ -f "$UPDATER_PUBLIC_KEY_PATH" ] || return 1
    cat "$UPDATER_PUBLIC_KEY_PATH"
}

load_release_notes() {
    if [ -n "$RELEASE_NOTES_OVERRIDE" ] && [ -n "$RELEASE_NOTES_FILE" ]; then
        die "Specify release notes either through RELEASE_NOTES or RELEASE_NOTES_FILE, not both."
    fi

    if [ -n "$RELEASE_NOTES_OVERRIDE" ]; then
        printf '%s' "$RELEASE_NOTES_OVERRIDE"
        return 0
    fi

    if [ -n "$RELEASE_NOTES_FILE" ]; then
        [ -f "$RELEASE_NOTES_FILE" ] || die "Release notes file not found: $RELEASE_NOTES_FILE"
        cat "$RELEASE_NOTES_FILE"
        return 0
    fi

    return 1
}

main() {
    local requested_name="${1:-}"
    local requested_version="${2:-}"
    local extra_arg="${3:-}"
    local dmg_name=""
    local app_version=""
    local updater_public_key=""
    local updater_endpoint=""
    local release_notes=""
    local release_download_base=""
    local dmg_bundle_dir=""
    local updater_bundle_dir=""
    local source_dmg=""
    local source_updater_bundle=""
    local source_updater_signature=""
    local output_dmg=""
    local output_updater_bundle=""
    local output_updater_signature=""
    local output_latest_json=""
    local updater_asset_name=""
    local updater_signature_contents=""
    local release_pub_date=""
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
    else
        app_version="$(normalize_version "$(read_checked_in_version)")"
    fi
    dmg_bundle_dir="$ROOT_DIR/src-tauri/target/$TARGET_TRIPLE/release/bundle/dmg"
    updater_bundle_dir="$ROOT_DIR/src-tauri/target/$TARGET_TRIPLE/release/bundle/macos"
    output_dmg="$OUTPUT_DIR/$dmg_name.dmg"
    output_latest_json="$OUTPUT_DIR/latest.json"

    [ "$(uname -s)" = "Darwin" ] || die "release.sh must be run on macOS."
    [ "$(uname -m)" = "arm64" ] || die "release.sh is configured for Apple Silicon hosts."
    [ -n "$GITHUB_RELEASE_TAG" ] || die "Set GITHUB_RELEASE_TAG to the GitHub release tag that will host the release assets."
    [ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ] || [ -n "${TAURI_SIGNING_PRIVATE_KEY_PATH:-}" ] || die "Set TAURI_SIGNING_PRIVATE_KEY or TAURI_SIGNING_PRIVATE_KEY_PATH so Tauri can sign updater artifacts."

    require_command npm
    require_command cargo
    require_command node

    if [ ! -d "$ROOT_DIR/node_modules" ]; then
        echo "Installing npm dependencies..."
        (cd "$ROOT_DIR" && npm install)
    fi

    updater_public_key="$(load_updater_public_key)" || die "Set OPEN_DUCK_UPDATER_PUBLIC_KEY or create $UPDATER_PUBLIC_KEY_PATH with your Tauri updater public key."
    updater_endpoint="${UPDATER_ENDPOINT_OVERRIDE:-https://github.com/$GITHUB_REPOSITORY/releases/latest/download/latest.json}"
    release_notes="$(load_release_notes || true)"
    release_download_base="https://github.com/$GITHUB_REPOSITORY/releases/download/$GITHUB_RELEASE_TAG"

    mkdir -p "$OUTPUT_DIR"

    echo "Building OpenDuck release bundle for $TARGET_TRIPLE..."
    TEMP_TAURI_CONFIG="$(mktemp "${TMPDIR:-/tmp}/openduck-tauri-release.XXXXXX.json")"
    trap 'rm -f "${TEMP_TAURI_CONFIG:-}"' EXIT
    printf '{\n  "version": "%s",\n  "bundle": {\n    "createUpdaterArtifacts": true\n  }\n}\n' "$app_version" >"$TEMP_TAURI_CONFIG"
    config_args=(--config "$TEMP_TAURI_CONFIG")
    echo "Using app version for this build: $app_version"
    echo "Using updater endpoint for this build: $updater_endpoint"
    build_env+=("OPEN_DUCK_BUILD_VERSION=$app_version")
    build_env+=("OPEN_DUCK_UPDATER_PUBLIC_KEY=$updater_public_key")
    build_env+=("OPEN_DUCK_UPDATER_ENDPOINT=$updater_endpoint")
    [ -z "${TAURI_SIGNING_PUBLIC_KEY:-}" ] || build_env+=("TAURI_SIGNING_PUBLIC_KEY=$TAURI_SIGNING_PUBLIC_KEY")
    [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ] || build_env+=("TAURI_SIGNING_PRIVATE_KEY=$TAURI_SIGNING_PRIVATE_KEY")
    [ -z "${TAURI_SIGNING_PRIVATE_KEY_PATH:-}" ] || build_env+=("TAURI_SIGNING_PRIVATE_KEY_PATH=$TAURI_SIGNING_PRIVATE_KEY_PATH")
    [ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ] || build_env+=("TAURI_SIGNING_PRIVATE_KEY_PASSWORD=$TAURI_SIGNING_PRIVATE_KEY_PASSWORD")

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
        (cd "$ROOT_DIR" && env "${build_env[@]}" npm run tauri build -- --target "$TARGET_TRIPLE" --bundles dmg,app "${config_args[@]}" "${build_args_array[@]}")
    else
        (cd "$ROOT_DIR" && env "${build_env[@]}" npm run tauri build -- --target "$TARGET_TRIPLE" --bundles dmg,app "${config_args[@]}")
    fi

    source_dmg="$(find_latest_matching "$dmg_bundle_dir" "*.dmg")" || die "No DMG was produced in $dmg_bundle_dir"
    source_updater_bundle="$(find_latest_matching "$updater_bundle_dir" "*.app.tar.gz")" || die "No updater bundle was produced in $updater_bundle_dir"
    source_updater_signature="$(find_latest_matching "$updater_bundle_dir" "*.app.tar.gz.sig")" || die "No updater signature was produced in $updater_bundle_dir"

    cp -f "$source_dmg" "$output_dmg"
    output_updater_bundle="$OUTPUT_DIR/$(basename "$source_updater_bundle")"
    output_updater_signature="$OUTPUT_DIR/$(basename "$source_updater_signature")"
    cp -f "$source_updater_bundle" "$output_updater_bundle"
    cp -f "$source_updater_signature" "$output_updater_signature"

    updater_asset_name="$(basename "$output_updater_bundle")"
    updater_signature_contents="$(tr -d '\r\n' <"$output_updater_signature")"
    [ -n "$updater_signature_contents" ] || die "Updater signature file was empty: $output_updater_signature"
    release_pub_date="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

    APP_VERSION="$app_version" \
        GITHUB_RELEASE_TAG="$GITHUB_RELEASE_TAG" \
        LATEST_JSON_PATH="$output_latest_json" \
        RELEASE_NOTES="$release_notes" \
        RELEASE_PUB_DATE="$release_pub_date" \
        TARGET_KEY="darwin-aarch64" \
        UPDATE_SIGNATURE="$updater_signature_contents" \
        UPDATE_URL="$release_download_base/$updater_asset_name" \
        node -e "const fs=require('fs'); const manifest={version:process.env.APP_VERSION,pub_date:process.env.RELEASE_PUB_DATE,platforms:{[process.env.TARGET_KEY]:{signature:process.env.UPDATE_SIGNATURE,url:process.env.UPDATE_URL}}}; if (process.env.RELEASE_NOTES) manifest.notes=process.env.RELEASE_NOTES; fs.writeFileSync(process.env.LATEST_JSON_PATH, JSON.stringify(manifest, null, 2) + '\n');"

    echo "Release artifacts ready:"
    echo "$output_dmg"
    echo "$output_updater_bundle"
    echo "$output_updater_signature"
    echo "$output_latest_json"
    echo
    echo "Upload these files to GitHub release tag $GITHUB_RELEASE_TAG:"
    echo "  - $(basename "$output_dmg")"
    echo "  - $(basename "$output_updater_bundle")"
    echo "  - $(basename "$output_updater_signature")"
    echo "  - $(basename "$output_latest_json")"
}

main "$@"
