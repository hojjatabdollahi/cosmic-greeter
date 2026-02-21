#!/usr/bin/env bash
# vm-start.sh - Boot existing Pop!_OS VM with SSH and VNC access
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
VM_DATA="$PROJECT_DIR/../cosmic-greeter-data"

# Configuration
VM_MEMORY="${VM_MEMORY:-4G}"
VM_CPUS="${VM_CPUS:-4}"
SSH_PORT="${SSH_PORT:-2222}"
VNC_DISPLAY="${VNC_DISPLAY:-0}"
VM_RESOLUTION="${VM_RESOLUTION:-1920x1080}"
DISK_FILE="$VM_DATA/popos.qcow2"

# Check disk exists
if [[ ! -f "$DISK_FILE" ]]; then
    echo "Error: Disk image not found: $DISK_FILE"
    echo "Run ./scripts/vm-create.sh first to create and install Pop!_OS"
    exit 1
fi

echo "=== Starting Pop!_OS VM ==="
echo "SSH:        ssh -p $SSH_PORT localhost"
echo "Display:    GTK window with OpenGL acceleration"
echo "Resolution: $VM_RESOLUTION"
echo ""
echo "Press Ctrl+C to stop the VM"
echo ""

exec qemu-system-x86_64 \
    -enable-kvm \
    -m "$VM_MEMORY" \
    -smp "$VM_CPUS" \
    -cpu host \
    -drive file="$DISK_FILE",format=qcow2,if=virtio \
    -netdev user,id=net0,hostfwd=tcp::${SSH_PORT}-:22 \
    -device virtio-net-pci,netdev=net0 \
    -device virtio-vga-gl,xres=${VM_RESOLUTION%x*},yres=${VM_RESOLUTION#*x} \
    -display gtk,gl=on \
    -usb \
    -device usb-tablet
