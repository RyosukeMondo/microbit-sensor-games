#!/bin/bash
# Setup script for BBC micro:bit on Linux
# This script configures udev rules and user permissions for micro:bit access

set -e

echo "=== BBC micro:bit Linux Setup ==="
echo ""

# Check if running with sudo
if [ "$EUID" -ne 0 ]; then
    echo "ERROR: Please run with sudo"
    echo "Usage: sudo ./setup.sh"
    exit 1
fi

# Get the actual user (not root when using sudo)
ACTUAL_USER="${SUDO_USER:-$USER}"
echo "Configuring for user: $ACTUAL_USER"
echo ""

# Create udev rules file
UDEV_RULES_FILE="/etc/udev/rules.d/99-microbit.rules"
echo "Creating udev rules at $UDEV_RULES_FILE"

cat > "$UDEV_RULES_FILE" << 'EOF'
# BBC micro:bit USB Rules
# Vendor ID: 0x0d28, Product ID: 0x0204

# USB device (general access)
SUBSYSTEM=="usb", ATTR{idVendor}=="0d28", ATTR{idProduct}=="0204", MODE="0666", GROUP="plugdev"

# HID device (for WebUSB/CMSIS-DAP debugging)
KERNEL=="hidraw*", ATTRS{idVendor}=="0d28", ATTRS{idProduct}=="0204", MODE="0666", GROUP="plugdev"

# Serial device (CDC-ACM) - disable ModemManager interference
SUBSYSTEM=="tty", ATTRS{idVendor}=="0d28", ATTRS{idProduct}=="0204", MODE="0666", GROUP="plugdev", ENV{ID_MM_DEVICE_IGNORE}="1"
EOF

echo "✓ udev rules created"
echo ""

# Create plugdev group if it doesn't exist
if ! getent group plugdev > /dev/null 2>&1; then
    echo "Creating plugdev group..."
    groupadd plugdev
    echo "✓ plugdev group created"
else
    echo "✓ plugdev group already exists"
fi

# Add user to required groups
echo "Adding $ACTUAL_USER to required groups..."

# Add to dialout group (for serial port access)
if ! groups "$ACTUAL_USER" | grep -q dialout; then
    usermod -a -G dialout "$ACTUAL_USER"
    echo "✓ Added to dialout group"
else
    echo "✓ Already in dialout group"
fi

# Add to plugdev group (for USB device access)
if ! groups "$ACTUAL_USER" | grep -q plugdev; then
    usermod -a -G plugdev "$ACTUAL_USER"
    echo "✓ Added to plugdev group"
else
    echo "✓ Already in plugdev group"
fi

echo ""

# Reload udev rules
echo "Reloading udev rules..."
udevadm control --reload-rules
udevadm trigger
echo "✓ udev rules reloaded"
echo ""

# Check if micro:bit is connected
if lsusb | grep -q "0d28:0204"; then
    echo "✓ micro:bit detected on USB"

    # Show device info
    echo ""
    echo "Device information:"
    lsusb | grep "0d28:0204"

    if [ -e /dev/ttyACM0 ]; then
        echo ""
        echo "Serial port: /dev/ttyACM0"
        ls -l /dev/ttyACM0
    fi
else
    echo "⚠ micro:bit not currently connected"
fi

echo ""
echo "=== Setup Complete ==="
echo ""
echo "IMPORTANT: You must log out and log back in (or reboot) for group changes to take effect."
echo ""
echo "After logging back in, verify your groups with:"
echo "  groups"
echo ""
echo "You should see both 'dialout' and 'plugdev' in the output."
echo ""
