#!/bin/bash
set -e

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Define directories
RESOURCES_DIR="$REPO_ROOT/src-tauri/resources"
PYTHON_ENV_DIR="$RESOURCES_DIR/python_env"
CSM_ENV_DIR="$RESOURCES_DIR/csm_env"
KOKORO_ENV_DIR="$RESOURCES_DIR/kokoro_env"
PORTABLE_PYTHON_TAR="$REPO_ROOT/python-standalone.tar.gz"
MLX_VLM_REPO="https://github.com/Blaizzy/mlx-vlm.git"
MLX_VLM_REF="${MLX_VLM_REF:-23e1dffd224488141a4f022b6d21d6a730f11507}"
CSM_MLX_REPO="https://github.com/senstella/csm-mlx"
MLX_AUDIO_VERSION="${MLX_AUDIO_VERSION:-0.2.5}"
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
if [ -d "$CSM_ENV_DIR" ]; then
    echo "Removing old CSM environment..."
    rm -rf "$CSM_ENV_DIR"
fi
if [ -d "$KOKORO_ENV_DIR" ]; then
    echo "Removing old Kokoro environment..."
    rm -rf "$KOKORO_ENV_DIR"
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

# Create isolated virtual environments for Gemma, CSM, and Kokoro.
echo "Creating Gemma virtual environment..."
"$PYTHON_ENV_DIR/bin/python3" -m venv "$PYTHON_ENV_DIR/venv"
echo "Creating CSM virtual environment..."
"$PYTHON_ENV_DIR/bin/python3" -m venv "$CSM_ENV_DIR/venv"
echo "Creating Kokoro virtual environment..."
"$PYTHON_ENV_DIR/bin/python3" -m venv "$KOKORO_ENV_DIR/venv"

# Install Gemma server dependencies into the Gemma venv.
echo "Installing mlx-vlm into the Gemma environment..."
"$PYTHON_ENV_DIR/venv/bin/pip" install -U pip
"$PYTHON_ENV_DIR/venv/bin/pip" install soundfile
echo "Installing mlx-vlm from $MLX_VLM_REPO @ $MLX_VLM_REF..."
"$PYTHON_ENV_DIR/venv/bin/pip" install "git+$MLX_VLM_REPO@$MLX_VLM_REF"

# Install speech backends into isolated venvs because mlx-audio and csm-mlx
# require different MLX dependency ranges.
echo "Installing csm-mlx into the CSM environment..."
"$CSM_ENV_DIR/venv/bin/pip" install -U pip
"$CSM_ENV_DIR/venv/bin/pip" install "git+$CSM_MLX_REPO" --upgrade
echo "Installing mlx-audio into the Kokoro environment..."
"$KOKORO_ENV_DIR/venv/bin/pip" install -U pip
"$KOKORO_ENV_DIR/venv/bin/pip" install "mlx-audio==$MLX_AUDIO_VERSION" soundfile
echo "Installing Kokoro English G2P model..."
"$KOKORO_ENV_DIR/venv/bin/python3" -m spacy download en_core_web_sm

echo "Setup complete!"
echo "Gemma environment: $PYTHON_ENV_DIR/venv"
echo "CSM environment: $CSM_ENV_DIR/venv"
echo "Kokoro environment: $KOKORO_ENV_DIR/venv"
