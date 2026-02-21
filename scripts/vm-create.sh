#!/usr/bin/env bash
# vm-create.sh - Download Pop!_OS ISO, create disk, and boot installer
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
VM_DATA="$PROJECT_DIR/../cosmic-greeter-data"

# Configuration
DISK_SIZE="${DISK_SIZE:-40G}"
VM_MEMORY="${VM_MEMORY:-4G}"
VM_CPUS="${VM_CPUS:-4}"
VM_RESOLUTION="${VM_RESOLUTION:-1920x1080}"
ISO_URL="${ISO_URL:-https://iso.pop-os.org/24.04/amd64/generic/22/pop-os_24.04_amd64_generic_22.iso}"
ISO_FILE="$VM_DATA/pop-os.iso"
DISK_FILE="$VM_DATA/popos.qcow2"

echo "=== cosmic-greeter VM Creator ==="
echo "VM Data directory: $VM_DATA"

# Create vm-data directory
mkdir -p "$VM_DATA"

# Download ISO if not present
if [[ ! -f "$ISO_FILE" ]]; then
  echo "Downloading Pop!_OS ISO..."
  echo "URL: $ISO_URL"
  wget -O "$ISO_FILE" "$ISO_URL"
else
  echo "ISO already exists: $ISO_FILE"
fi

# Create disk image if not present
if [[ ! -f "$DISK_FILE" ]]; then
  echo "Creating $DISK_SIZE disk image..."
  qemu-img create -f qcow2 "$DISK_FILE" "$DISK_SIZE"
else
  echo "Disk image already exists: $DISK_FILE"
  echo "Delete it manually if you want to start fresh: rm $DISK_FILE"
fi

echo ""
echo "=== Starting VM Installer ==="
echo "A GTK window will open with the installer."
echo "Install Pop!_OS, then shut down the VM."
echo ""
echo "After installation, remember to:"
echo "  1. Boot the VM with: ./scripts/vm-start.sh"
echo "  2. Install SSH: sudo apt install openssh-server"
echo "  3. Enable SSH: sudo systemctl enable --now ssh"
echo ""

# Boot VM with ISO
exec qemu-system-x86_64 \
  -enable-kvm \
  -m "$VM_MEMORY" \
  -smp "$VM_CPUS" \
  -cpu host \
  -drive file="$DISK_FILE",format=qcow2,if=virtio \
  -cdrom "$ISO_FILE" \
  -boot d \
  -netdev user,id=net0,hostfwd=tcp::2222-:22 \
  -device virtio-net-pci,netdev=net0 \
  -device virtio-vga-gl,xres=${VM_RESOLUTION%x*},yres=${VM_RESOLUTION#*x} \
  -display gtk,gl=on \
  -usb \
  -device usb-tablet
