#!/usr/bin/env bash
# vm-deploy-settings.sh - Build cosmic-settings, deploy to VM
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
COSMIC_SETTINGS_DIR="${COSMIC_SETTINGS_DIR:-$PROJECT_DIR/../cosmic-settings}"

# Configuration
SSH_PORT="${SSH_PORT:-2222}"
SSH_HOST="${SSH_HOST:-localhost}"
VM_USER="${VM_USER:-}"
BUILD="${BUILD:-1}"

# Binary to deploy
SETTINGS_BIN="$COSMIC_SETTINGS_DIR/target/release/cosmic-settings"

# Check for VM_USER
if [[ -z "$VM_USER" ]]; then
    echo "Error: VM_USER environment variable not set"
    echo "Usage: VM_USER=youruser ./scripts/vm-deploy-settings.sh"
    exit 1
fi

# Check cosmic-settings directory exists
if [[ ! -d "$COSMIC_SETTINGS_DIR" ]]; then
    echo "Error: cosmic-settings directory not found: $COSMIC_SETTINGS_DIR"
    echo "Set COSMIC_SETTINGS_DIR to the correct path"
    exit 1
fi

SSH_TARGET="$VM_USER@$SSH_HOST"
SSH_CMD="ssh -t -p $SSH_PORT $SSH_TARGET"

echo "=== cosmic-settings VM Deployer ==="
echo "Target: $SSH_TARGET:$SSH_PORT"
echo "cosmic-settings dir: $COSMIC_SETTINGS_DIR"
echo ""

# Build if requested
if [[ "$BUILD" == "1" ]]; then
    echo "Building cosmic-settings (release)..."
    (cd "$COSMIC_SETTINGS_DIR" && cargo build --release)
    echo ""
fi

# Check binary exists
if [[ ! -f "$SETTINGS_BIN" ]]; then
    echo "Error: cosmic-settings binary not found: $SETTINGS_BIN"
    echo "Run 'cargo build --release' in cosmic-settings directory first"
    exit 1
fi

echo "Copying binary to VM..."
scp -P "$SSH_PORT" "$SETTINGS_BIN" "$SSH_TARGET:/tmp/cosmic-settings-new"

echo "Installing binary..."
$SSH_CMD 'set -e; echo "Removing old binary..."; sudo rm -f /usr/bin/cosmic-settings; echo "Installing new binary..."; sudo mv /tmp/cosmic-settings-new /usr/bin/cosmic-settings; sudo chmod +x /usr/bin/cosmic-settings; echo "Done! Restart cosmic-settings or your session to see changes"'

echo ""
echo "=== Deployment Complete ==="
echo "cosmic-settings has been installed."
echo "Launch it from the app menu or run: cosmic-settings"
