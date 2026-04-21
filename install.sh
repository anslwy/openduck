#!/bin/bash
set -euo pipefail

echo "Checking dependencies..."

# Check for Rust/Cargo
if ! command -v cargo &> /dev/null; then
    echo "Rust/Cargo not found. Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    
    # Load cargo environment for the current session
    if [ -f "$HOME/.cargo/env" ]; then
        source "$HOME/.cargo/env"
    fi
else
    echo "Rust/Cargo already installed: $(cargo --version)"
fi

# Check for Node.js
if ! command -v npm &> /dev/null; then
    echo "Error: npm not found. Please install Node.js (https://nodejs.org/)"
    exit 1
else
    echo "Node.js/npm already installed: $(npm --version)"
fi

echo "Installing Node.js dependencies..."
npm install

echo "Setting up Python environment (this may take a few minutes)..."
./scripts/setup_python_env.sh

echo ""
echo "Installation complete!"
echo "If you just installed Rust, you might need to restart your terminal or run: source \$HOME/.cargo/env"
echo "Then you can start the app with: ./start.sh"
