#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"

pkill -f "$ROOT_DIR/src-tauri/resources/patch_mlx_vlm.py" >/dev/null 2>&1 || true
pkill -f "$ROOT_DIR/src-tauri/resources/csm_stream.py" >/dev/null 2>&1 || true

cd "$ROOT_DIR"
npm run tauri dev
