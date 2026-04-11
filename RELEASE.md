# Release

Build an Apple Silicon DMG from the repo root with:

```bash
./scripts/release.sh openduck-beta-v0
```

That writes the final artifact to:

```bash
dist/openduck-beta-v0.dmg
```

## Useful Variants

Skip code signing during the Tauri bundle step:

```bash
TAURI_BUILD_ARGS="--no-sign" ./scripts/release.sh openduck-beta-v1
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
- The packaged app now prepares its Python runtime on first launch instead of bundling the generated venv directories into the DMG.
- First launch still needs internet access, and it may need Apple Command Line Tools if `git` is not already available because the runtime setup installs a couple of GitHub-hosted Python packages.
- The generated DMG is built by Tauri first, then copied to `dist/<name>.dmg`.
