# Release

OpenDuck uses Tauri's signed updater flow for in-app update checks.

That means a release build is no longer just a DMG upload. Each GitHub Release needs four assets:

- the DMG for manual installs
- the macOS updater bundle: `OpenDuck.app.tar.gz`
- the updater signature: `OpenDuck.app.tar.gz.sig`
- `latest.json`, which tells the app which updater bundle to download

The app checks GitHub at:

```text
https://github.com/anslwy/openduck/releases/latest/download/latest.json
```

So whichever GitHub Release is marked as the latest release controls what the app updates to.

## One-Time Setup

Generate a Tauri updater signing key:

```bash
npm run tauri signer generate -- -w ~/.tauri/openduck.key
```

Then:

1. Keep the private key outside the repo.
2. Save the generated public key contents to `src-tauri/updater-public-key.pem`.

The public key is safe to commit. The private key is not.

Before building releases, export your signing key:

```bash
export TAURI_SIGNING_PRIVATE_KEY="$HOME/.tauri/openduck.key"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
```

Newer Tauri CLIs also support `TAURI_SIGNING_PRIVATE_KEY_PATH`.

## Build

Build a release bundle from the repo root with:

```bash
GITHUB_RELEASE_TAG=v1.2.3 ./scripts/release.sh openduck-1.2.3
```

That writes these files to `dist/`:

- `openduck-1.2.3.dmg`
- `OpenDuck.app.tar.gz`
- `OpenDuck.app.tar.gz.sig`
- `latest.json`

Upload all four files to the GitHub Release whose tag matches `GITHUB_RELEASE_TAG`.

By default, `release.sh` uses the app version already checked into the repo metadata.
If the build runs inside a Git checkout, the app also stamps the current Git commit SHA into OpenDuck's custom About dialog.

## Versioning

The DMG file name, GitHub release tag, and app version are separate values.

If you want a one-off release version without editing tracked files, pass the app version as the second argument:

```bash
GITHUB_RELEASE_TAG=v1.2.3 ./scripts/release.sh openduck-1.2.3 1.2.3
```

You can also provide it through `VERSION`:

```bash
GITHUB_RELEASE_TAG=v1.2.3 VERSION=1.2.3 ./scripts/release.sh openduck-1.2.3
```

That version override is applied only for the current Tauri build. It does not rewrite any repo files.

## About Metadata

OpenDuck now shows release metadata in its custom About dialog instead of relying only on the stock macOS version panel.

By default, the build metadata includes:

- app version
- current Git commit SHA, if Git metadata is available at build time
- a derived build id when channel, label, build number, or Git SHA are present

You can add a human-readable release label such as `Beta`:

```bash
GITHUB_RELEASE_TAG=v1.2.3 VERSION=1.2.3 VERSION_LABEL="Beta" ./scripts/release.sh openduck-beta-1.2.3
```

You can also attach channel/build metadata that gets folded into the custom About dialog and build id:

```bash
GITHUB_RELEASE_TAG=v0.0.2 VERSION=0.0.2 VERSION_LABEL="Beta" BUILD_CHANNEL=beta BUILD_NUMBER=221 ./scripts/release.sh openduck-beta-v0.0.2
```

Typical build id shape:

```text
1.2.3+beta.221.<git-sha>
```

Notes:

- `VERSION_LABEL` is display text for the custom About dialog, for example `Beta`.
- `BUILD_CHANNEL` is a machine-readable identifier used in the build id, for example `beta` or `stable`.
- `BUILD_NUMBER` is an optional sequence/build number.
- Direct `tauri build` runs can set `VERSION_LABEL`, `BUILD_CHANNEL`, and `BUILD_NUMBER` directly; `release.sh` forwards them to the embedded `OPEN_DUCK_*` build metadata env vars.
- Human labels do not rewrite the bundle version field shown by Finder metadata; they are shown in OpenDuck's custom About dialog and copyable build info.

If you want to permanently bump the checked-in app version, keep these files in sync:

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

Example persistent bump:

1. Change the app version in all three files from `1.0.0` to `1.2.3`.
2. Build the DMG with a matching file name:

```bash
GITHUB_RELEASE_TAG=v1.2.3 ./scripts/release.sh openduck-1.2.3
```

Current limitation:

- Set the override with either the second argument or `VERSION`, not both.
- If the build is not running from a Git checkout and `OPEN_DUCK_GIT_SHA` is not provided, the commit SHA will be unavailable.
- The app only updates to the GitHub Release that serves `releases/latest/download/latest.json`.
- `release.sh` requires a signing key and updater public key because updater artifacts must be signed.

## Useful Variants

Skip code signing during the Tauri bundle step:

```bash
GITHUB_RELEASE_TAG=v1.2.3 TAURI_BUILD_ARGS="--no-sign" ./scripts/release.sh openduck-beta-v1
```

Skip signing and override the app version for a single build:

```bash
GITHUB_RELEASE_TAG=v1.2.3 TAURI_BUILD_ARGS="--no-sign" ./scripts/release.sh openduck-beta-v1 1.2.3
```

Skip signing while stamping a beta label and build number:

```bash
GITHUB_RELEASE_TAG=v1.2.3 TAURI_BUILD_ARGS="--no-sign" VERSION=1.2.3 VERSION_LABEL="Beta" BUILD_CHANNEL=beta BUILD_NUMBER=221 ./scripts/release.sh openduck-beta-v1
```

Write the DMG to a custom directory:

```bash
GITHUB_RELEASE_TAG=v1.2.3 OUTPUT_DIR="$HOME/Desktop/openduck-releases" ./scripts/release.sh openduck-beta-v2
```

Override the build target triple:

```bash
GITHUB_RELEASE_TAG=v1.2.3 TARGET_TRIPLE=aarch64-apple-darwin ./scripts/release.sh openduck-beta-v3
```

Add release notes to the updater manifest:

```bash
GITHUB_RELEASE_TAG=v1.2.3 RELEASE_NOTES_FILE=release-notes.md ./scripts/release.sh openduck-1.2.3
```

## Debugging DMG Failures

If Tauri only prints a generic `failed to run ... bundle_dmg.sh` error, use the local verbose helpers:

```bash
npm run tauri:build:verbose -- --target aarch64-apple-darwin --bundles dmg,app
```

If the DMG error is still opaque, replay the generated DMG command directly with verbose `hdiutil` output:

```bash
npm run tauri:dmg:debug
```

That helper writes a timestamped log under:

```text
src-tauri/target/<target>/release/bundle/dmg/bundle_dmg_debug_<timestamp>.log
```

It requires a prior Tauri build attempt so the generated `bundle_dmg.sh` and `.app` bundle already exist.

## Notes

- Run the script on macOS on an Apple Silicon machine.
- The script installs `node_modules` if needed.
- Version overrides and `createUpdaterArtifacts` are passed to Tauri via a temporary config file created during the build and removed on exit.
- Build metadata for the custom About dialog is passed to Rust through environment variables and finalized in `src-tauri/build.rs`.
- The updater public key is embedded into the app at build time from `OPEN_DUCK_UPDATER_PUBLIC_KEY` or `src-tauri/updater-public-key.pem`.
- `latest.json` is generated for `darwin-aarch64` and points at the exact GitHub Release tag you pass in via `GITHUB_RELEASE_TAG`.
- The packaged app now prepares its Python runtime on first launch instead of bundling the generated venv directories into the DMG.
- First launch still needs internet access to download the local Python runtime and models (this can take several minutes). Once setup is complete, the app can be used offline.
- The generated DMG and updater artifacts are built by Tauri first, then copied to `dist/`.
