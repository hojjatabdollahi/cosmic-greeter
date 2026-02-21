#!/usr/bin/env bash
# vm-deploy-bg.sh - Build cosmic-bg, deploy to VM, and restart it
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
COSMIC_BG_DIR="${COSMIC_BG_DIR:-$PROJECT_DIR/../cosmic-bg}"

# Configuration
SSH_PORT="${SSH_PORT:-2222}"
SSH_HOST="${SSH_HOST:-localhost}"
VM_USER="${VM_USER:-}"
BUILD="${BUILD:-1}"

# Binary and shaders to deploy
BG_BIN="$COSMIC_BG_DIR/target/release/cosmic-bg"
SHADERS_DIR="$COSMIC_BG_DIR/examples"
SHADERS_DST="/usr/share/cosmic-bg/shaders"

# Check for VM_USER
if [[ -z "$VM_USER" ]]; then
    echo "Error: VM_USER environment variable not set"
    echo "Usage: VM_USER=youruser ./scripts/vm-deploy-bg.sh"
    exit 1
fi

# Check cosmic-bg directory exists
if [[ ! -d "$COSMIC_BG_DIR" ]]; then
    echo "Error: cosmic-bg directory not found: $COSMIC_BG_DIR"
    echo "Set COSMIC_BG_DIR to the correct path"
    exit 1
fi

SSH_TARGET="$VM_USER@$SSH_HOST"
SSH_CMD="ssh -t -p $SSH_PORT $SSH_TARGET"

echo "=== cosmic-bg VM Deployer ==="
echo "Target: $SSH_TARGET:$SSH_PORT"
echo "cosmic-bg dir: $COSMIC_BG_DIR"
echo ""

# Build if requested
if [[ "$BUILD" == "1" ]]; then
    echo "Building cosmic-bg (release)..."
    (cd "$COSMIC_BG_DIR" && just build-release)
    echo ""
fi

# Check binary exists
if [[ ! -f "$BG_BIN" ]]; then
    echo "Error: cosmic-bg binary not found: $BG_BIN"
    echo "Run 'just build-release' in cosmic-bg directory first"
    exit 1
fi

echo "Copying binary to VM..."
scp -P "$SSH_PORT" "$BG_BIN" "$SSH_TARGET:/tmp/cosmic-bg-new"

echo "Copying shaders to VM..."
scp -P "$SSH_PORT" "$SHADERS_DIR"/*.wgsl "$SSH_TARGET:/tmp/"

echo "Installing binary and shaders..."
$SSH_CMD "set -e; echo 'Removing old binary...'; sudo rm -f /usr/bin/cosmic-bg; echo 'Installing new binary...'; sudo mv /tmp/cosmic-bg-new /usr/bin/cosmic-bg; sudo chmod +x /usr/bin/cosmic-bg; echo 'Installing shaders...'; sudo mkdir -p $SHADERS_DST; sudo mv /tmp/*.wgsl $SHADERS_DST/; sudo chmod 644 $SHADERS_DST/*.wgsl; echo 'Done!'"

echo ""
echo "=== Deployment Complete ==="
echo "cosmic-bg has been installed. Restart greetd or your session to see changes:"
echo "  $SSH_CMD 'sudo systemctl restart greetd'"
