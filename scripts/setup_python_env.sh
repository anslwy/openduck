#!/bin/bash
set -e

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Define directories
RESOURCES_DIR="$REPO_ROOT/src-tauri/resources"
PYTHON_ENV_DIR="$RESOURCES_DIR/python_env"
PORTABLE_PYTHON_TAR="$REPO_ROOT/python-standalone.tar.gz"
MLX_VLM_REPO="https://github.com/Blaizzy/mlx-vlm.git"
MLX_VLM_REF="${MLX_VLM_REF:-23e1dffd224488141a4f022b6d21d6a730f11507}"
PATCH_SERVER_SCRIPT="$RESOURCES_DIR/patch_mlx_vlm.py"

echo "Stopping existing MLX server processes..."
SERVER_PIDS="$(pgrep -f "$PATCH_SERVER_SCRIPT" || true)"
if [ -n "$SERVER_PIDS" ]; then
    for pid in $SERVER_PIDS; do
        kill "$pid" 2>/dev/null || true
    done
    sleep 1

    SERVER_PIDS="$(pgrep -f "$PATCH_SERVER_SCRIPT" || true)"
    if [ -n "$SERVER_PIDS" ]; then
        for pid in $SERVER_PIDS; do
            kill -9 "$pid" 2>/dev/null || true
        done
    fi
fi

# Clean up existing environment if it exists to ensure a clean 3.11 install
if [ -d "$PYTHON_ENV_DIR" ]; then
    echo "Removing old Python environment..."
    rm -rf "$PYTHON_ENV_DIR"
fi

mkdir -p "$RESOURCES_DIR"

# Download a portable Python 3.11 for macOS (M1/M2/M3)
# Using indygreg's python-build-standalone
echo "Downloading portable Python 3.11..."
# Link for aarch64-apple-darwin
PYTHON_URL="https://github.com/indygreg/python-build-standalone/releases/download/20240107/cpython-3.11.7+20240107-aarch64-apple-darwin-install_only.tar.gz"

curl -L "$PYTHON_URL" -o "$PORTABLE_PYTHON_TAR"

# Extract Python
echo "Extracting Python..."
mkdir -p "$PYTHON_ENV_DIR"
tar -xzf "$PORTABLE_PYTHON_TAR" -C "$PYTHON_ENV_DIR" --strip-components=1

# Clean up tarball
rm "$PORTABLE_PYTHON_TAR"

# Create a virtual environment using the portable Python
echo "Creating virtual environment..."
"$PYTHON_ENV_DIR/bin/python3" -m venv "$PYTHON_ENV_DIR/venv"

# Use the venv's pip to install mlx-vlm and dependencies
echo "Installing mlx-vlm into the venv..."
"$PYTHON_ENV_DIR/venv/bin/pip" install -U pip
"$PYTHON_ENV_DIR/venv/bin/pip" install soundfile
echo "Installing mlx-vlm from $MLX_VLM_REPO @ $MLX_VLM_REF..."
"$PYTHON_ENV_DIR/venv/bin/pip" install "git+$MLX_VLM_REPO@$MLX_VLM_REF"

echo "Setup complete! Python 3.11 and mlx-vlm have been prepared in $PYTHON_ENV_DIR"
