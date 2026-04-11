# Release

Build an Apple Silicon DMG from the repo root with:

```bash
./scripts/release.sh openduck-beta-v0
```

That writes the final artifact to:

```bash
dist/openduck-beta-v0.dmg
```

By default, `release.sh` uses the app version already checked into the repo metadata.
If the build runs inside a Git checkout, the app also stamps the current Git commit SHA into OpenDuck's custom About dialog.

## Versioning

The DMG file name and the app version are separate values.

If you want a one-off release version without editing tracked files, pass the app version as the second argument:

```bash
./scripts/release.sh openduck-1.2.3 1.2.3
```

You can also provide it through `VERSION`:

```bash
VERSION=1.2.3 ./scripts/release.sh openduck-1.2.3
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
VERSION=1.2.3 VERSION_LABEL="Beta" ./scripts/release.sh openduck-beta-1.2.3
```

You can also attach channel/build metadata that gets folded into the custom About dialog and build id:

```bash
VERSION=1.2.3 VERSION_LABEL="Beta" BUILD_CHANNEL=beta BUILD_NUMBER=221 ./scripts/release.sh openduck-beta-221
```

Typical build id shape:

```text
1.2.3+beta.221.<git-sha>
```

Notes:

- `VERSION_LABEL` is display text for the custom About dialog, for example `Beta`.
- `BUILD_CHANNEL` is a machine-readable identifier used in the build id, for example `beta` or `stable`.
- `BUILD_NUMBER` is an optional sequence/build number.
- Human labels do not rewrite the bundle version field shown by Finder metadata; they are shown in OpenDuck's custom About dialog and copyable build info.

If you want to permanently bump the checked-in app version, keep these files in sync:

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

Example persistent bump:

1. Change the app version in all three files from `1.0.0` to `1.2.3`.
2. Build the DMG with a matching file name:

```bash
./scripts/release.sh openduck-1.2.3
```

Current limitation:

- Set the override with either the second argument or `VERSION`, not both.
- If the build is not running from a Git checkout and `OPEN_DUCK_GIT_SHA` is not provided, the commit SHA will be unavailable.

## Useful Variants

Skip code signing during the Tauri bundle step:

```bash
TAURI_BUILD_ARGS="--no-sign" ./scripts/release.sh openduck-beta-v1
```

Skip signing and override the app version for a single build:

```bash
TAURI_BUILD_ARGS="--no-sign" ./scripts/release.sh openduck-beta-v1 1.2.3
```

Skip signing while stamping a beta label and build number:

```bash
TAURI_BUILD_ARGS="--no-sign" VERSION=1.2.3 VERSION_LABEL="Beta" BUILD_CHANNEL=beta BUILD_NUMBER=221 ./scripts/release.sh openduck-beta-v1
```

Write the DMG to a custom directory:

```bash
OUTPUT_DIR="$HOME/Desktop/openduck-releases" ./scripts/release.sh openduck-beta-v2
```

Override the build target triple:

```bash
TARGET_TRIPLE=aarch64-apple-darwin ./scripts/release.sh openduck-beta-v3
```

## Notes

- Run the script on macOS on an Apple Silicon machine.
- The script installs `node_modules` if needed.
- Version overrides are passed to Tauri via a temporary config file created during the build and removed on exit.
- Build metadata for the custom About dialog is passed to Rust through environment variables and finalized in `src-tauri/build.rs`.
- The packaged app now prepares its Python runtime on first launch instead of bundling the generated venv directories into the DMG.
- First launch still needs internet access, and it may need Apple Command Line Tools if `git` is not already available because the runtime setup installs a couple of GitHub-hosted Python packages.
- The generated DMG is built by Tauri first, then copied to `dist/<name>.dmg`.
