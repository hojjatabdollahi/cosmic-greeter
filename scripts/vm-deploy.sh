#!/usr/bin/env bash
# vm-deploy.sh - Build cosmic-greeter, deploy to VM, and restart greetd
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Configuration
SSH_PORT="${SSH_PORT:-2222}"
SSH_HOST="${SSH_HOST:-localhost}"
VM_USER="${VM_USER:-}"
BUILD="${BUILD:-1}"

# Binaries to deploy
GREETER_BIN="$PROJECT_DIR/target/release/cosmic-greeter"
DAEMON_BIN="$PROJECT_DIR/target/release/cosmic-greeter-daemon"

# Check for VM_USER
if [[ -z "$VM_USER" ]]; then
    echo "Error: VM_USER environment variable not set"
    echo "Usage: VM_USER=youruser ./scripts/vm-deploy.sh"
    exit 1
fi

SSH_TARGET="$VM_USER@$SSH_HOST"
SSH_CMD="ssh -t -p $SSH_PORT $SSH_TARGET"

echo "=== cosmic-greeter VM Deployer ==="
echo "Target: $SSH_TARGET:$SSH_PORT"
echo ""

# Build if requested
if [[ "$BUILD" == "1" ]]; then
    echo "Building cosmic-greeter (release)..."
    (cd "$PROJECT_DIR" && just build-release)
    echo ""
fi

# Check binaries exist
if [[ ! -f "$GREETER_BIN" ]]; then
    echo "Error: cosmic-greeter binary not found: $GREETER_BIN"
    echo "Run 'just build-release' first"
    exit 1
fi

if [[ ! -f "$DAEMON_BIN" ]]; then
    echo "Error: cosmic-greeter-daemon binary not found: $DAEMON_BIN"
    echo "Run 'just build-release' first"
    exit 1
fi

echo "Copying binaries to VM..."
scp -P "$SSH_PORT" "$GREETER_BIN" "$SSH_TARGET:/tmp/cosmic-greeter-new"
scp -P "$SSH_PORT" "$DAEMON_BIN" "$SSH_TARGET:/tmp/cosmic-greeter-daemon-new"

echo "Installing binaries and restarting greetd..."
$SSH_CMD 'set -e; echo "Stopping greetd..."; sudo systemctl stop greetd || true; echo "Removing old binaries..."; sudo rm -f /usr/bin/cosmic-greeter /usr/bin/cosmic-greeter-daemon; echo "Installing new binaries..."; sudo mv /tmp/cosmic-greeter-new /usr/bin/cosmic-greeter; sudo mv /tmp/cosmic-greeter-daemon-new /usr/bin/cosmic-greeter-daemon; sudo chmod +x /usr/bin/cosmic-greeter /usr/bin/cosmic-greeter-daemon; echo "Starting greetd..."; sudo systemctl start greetd; echo "Done! greetd status:"; sudo systemctl status greetd --no-pager || true'

echo ""
echo "=== Deployment Complete ==="
echo "Connect via VNC to localhost:5900 to see the login screen"
echo ""
echo "Useful commands:"
echo "  View greetd logs: $SSH_CMD 'journalctl -u greetd -f'"
echo "  Restart greetd:   $SSH_CMD 'sudo systemctl restart greetd'"
