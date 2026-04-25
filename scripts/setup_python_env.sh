#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
STATUS_PREFIX="OPEN_DUCK_STATUS:"

status() {
    printf '%s %s\n' "$STATUS_PREFIX" "$1"
}

# Allow the packaged app to install its Python runtime into a writable
# application data directory instead of the read-only app bundle.
RESOURCE_FILES_DIR="${OPEN_DUCK_RESOURCE_FILES_DIR:-$REPO_ROOT/src-tauri/resources}"
RUNTIME_ROOT_DIR="${OPEN_DUCK_RUNTIME_ROOT:-$REPO_ROOT/src-tauri/resources}"
TEMP_DIR="${OPEN_DUCK_TEMP_DIR:-$REPO_ROOT}"

# Define directories
PYTHON_ENV_DIR="$RUNTIME_ROOT_DIR/python_env"
CSM_ENV_DIR="$RUNTIME_ROOT_DIR/csm_env"
KOKORO_ENV_DIR="$RUNTIME_ROOT_DIR/kokoro_env"
COSYVOICE_ENV_DIR="$RUNTIME_ROOT_DIR/cosyvoice_env"
STT_ENV_DIR="$RUNTIME_ROOT_DIR/stt_env"
PORTABLE_PYTHON_TAR="$TEMP_DIR/python-standalone.tar.gz"
MLX_VLM_REPO="https://github.com/Blaizzy/mlx-vlm.git"
MLX_VLM_REF="${MLX_VLM_REF:-23e1dffd224488141a4f022b6d21d6a730f11507}"
CSM_MLX_REPO="https://github.com/senstella/csm-mlx"
MLX_AUDIO_VERSION="${MLX_AUDIO_VERSION:-0.2.5}"
MLX_AUDIO_PLUS_VERSION="${MLX_AUDIO_PLUS_VERSION:-0.1.8}"
MLX_AUDIO_STT_VERSION="${MLX_AUDIO_STT_VERSION:-0.3.1}"
PATCH_SERVER_SCRIPT="${OPEN_DUCK_PATCH_SERVER_SCRIPT:-$RESOURCE_FILES_DIR/patch_mlx_vlm.py}"

export PIP_DISABLE_PIP_VERSION_CHECK=1
export PYTHONDONTWRITEBYTECODE=1

status "Stopping existing MLX server processes..."
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

# Clean up existing environment only if it is missing the core interpreter
if [ -d "$PYTHON_ENV_DIR" ] && [ ! -f "$PYTHON_ENV_DIR/bin/python3" ]; then
    status "Removing broken Python environment..."
    rm -rf "$PYTHON_ENV_DIR"
fi

mkdir -p "$RUNTIME_ROOT_DIR"
mkdir -p "$TEMP_DIR"

# Download a portable Python 3.11 for macOS (M1/M2/M3)
# Using indygreg's python-build-standalone
if [ ! -f "$PYTHON_ENV_DIR/bin/python3" ]; then
    status "Downloading portable Python 3.11..."
    # Link for aarch64-apple-darwin
    PYTHON_URL="https://github.com/indygreg/python-build-standalone/releases/download/20240107/cpython-3.11.7+20240107-aarch64-apple-darwin-install_only.tar.gz"

    curl -L "$PYTHON_URL" -o "$PORTABLE_PYTHON_TAR"

    # Extract Python
    status "Extracting Python..."
    mkdir -p "$PYTHON_ENV_DIR"
    tar -xzf "$PORTABLE_PYTHON_TAR" -C "$PYTHON_ENV_DIR" --strip-components=1

    # Clean up tarball
    rm "$PORTABLE_PYTHON_TAR"
else
    status "Portable Python 3.11 already installed."
fi

# Create isolated virtual environments for Gemma and speech backends.
# venv creation is fast if they already exist.
status "Ensuring Gemma virtual environment..."
"$PYTHON_ENV_DIR/bin/python3" -m venv "$PYTHON_ENV_DIR/venv"
status "Ensuring CSM virtual environment..."
"$PYTHON_ENV_DIR/bin/python3" -m venv "$CSM_ENV_DIR/venv"
status "Ensuring Kokoro virtual environment..."
"$PYTHON_ENV_DIR/bin/python3" -m venv "$KOKORO_ENV_DIR/venv"
status "Ensuring CosyVoice virtual environment..."
"$PYTHON_ENV_DIR/bin/python3" -m venv "$COSYVOICE_ENV_DIR/venv"
status "Ensuring STT virtual environment..."
"$PYTHON_ENV_DIR/bin/python3" -m venv "$STT_ENV_DIR/venv"

# Install Gemma server dependencies into the Gemma venv.
status "Installing mlx-vlm into the Gemma environment..."
"$PYTHON_ENV_DIR/venv/bin/pip" install -U pip
"$PYTHON_ENV_DIR/venv/bin/pip" install numpy huggingface_hub tqdm mlx-lm soundfile
status "Installing mlx-vlm from tarball @ $MLX_VLM_REF..."
"$PYTHON_ENV_DIR/venv/bin/pip" install "mlx-vlm @ https://github.com/Blaizzy/mlx-vlm/archive/$MLX_VLM_REF.tar.gz"

# Install speech backends into isolated venvs because the TTS stacks
# have different MLX dependency ranges and package names.
status "Installing csm-mlx into the CSM environment..."
"$CSM_ENV_DIR/venv/bin/pip" install -U pip
"$CSM_ENV_DIR/venv/bin/pip" install numpy huggingface_hub tqdm "csm-mlx @ https://github.com/senstella/csm-mlx/archive/master.tar.gz" --upgrade
status "Installing mlx-audio into the Kokoro environment..."
"$KOKORO_ENV_DIR/venv/bin/pip" install -U pip
"$KOKORO_ENV_DIR/venv/bin/pip" install numpy huggingface_hub tqdm spacy "mlx-audio==$MLX_AUDIO_VERSION" soundfile
status "Installing Kokoro multilingual G2P dependencies..."
"$KOKORO_ENV_DIR/venv/bin/pip" install "misaki[ja,zh]"
if ! "$KOKORO_ENV_DIR/venv/bin/python3" -c "from pathlib import Path; import unidic; raise SystemExit(0 if Path(unidic.DICDIR, 'mecabrc').exists() else 1)"; then
    status "Installing Kokoro Japanese UniDic dictionary..."
    "$KOKORO_ENV_DIR/venv/bin/python3" -m unidic download
else
    status "Kokoro Japanese UniDic dictionary already installed."
fi
status "Verifying Kokoro multilingual G2P dependencies..."
"$KOKORO_ENV_DIR/venv/bin/python3" -c "from pathlib import Path; import fugashi, jaconv, mojimoji, pyopenjtalk, unidic, cn2an, jieba, pypinyin, pypinyin_dict; assert Path(unidic.DICDIR, 'mecabrc').exists(), f'Missing UniDic dictionary at {unidic.DICDIR}'"
status "Installing Kokoro English G2P model..."
"$KOKORO_ENV_DIR/venv/bin/python3" -m spacy download en_core_web_sm
status "Installing mlx-audio-plus into the CosyVoice environment..."
"$COSYVOICE_ENV_DIR/venv/bin/pip" install -U pip
"$COSYVOICE_ENV_DIR/venv/bin/pip" install numpy huggingface_hub tqdm "mlx-audio-plus==$MLX_AUDIO_PLUS_VERSION" soundfile
status "Installing mlx-audio into the STT environment..."
"$STT_ENV_DIR/venv/bin/pip" install -U pip
"$STT_ENV_DIR/venv/bin/pip" install numpy huggingface_hub tqdm "mlx-audio==$MLX_AUDIO_STT_VERSION" soundfile

status "Setup complete!"
touch "$RUNTIME_ROOT_DIR/.complete" "$RUNTIME_ROOT_DIR/.kokoro-multilingual-v2"
status "Gemma environment: $PYTHON_ENV_DIR/venv"
status "CSM environment: $CSM_ENV_DIR/venv"
status "Kokoro environment: $KOKORO_ENV_DIR/venv"
status "CosyVoice environment: $COSYVOICE_ENV_DIR/venv"
status "STT environment: $STT_ENV_DIR/venv"
