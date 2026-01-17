#!/bin/bash
# RDBMS Installation Script
# Installs RDBMS as a systemd service

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SYSTEMD_DIR="/etc/systemd/system"
INSTALL_PREFIX="${INSTALL_PREFIX:-/usr/local}"
BIN_DIR="${INSTALL_PREFIX}/bin"
CONFIG_DIR="${CONFIG_DIR:-/etc/rdbms}"

echo "=== RDBMS Installation Script ==="
echo "Install prefix: $INSTALL_PREFIX"
echo "Repository root: $REPO_ROOT"

# Check if running as root
if [ "$(id -u)" -ne 0 ]; then
    echo "Error: This script must be run as root."
    echo "Please run with: sudo $0"
    exit 1
fi

# Step 1: Build if needed
echo ""
echo "Step 1: Building RDBMS..."
if [ ! -f "$REPO_ROOT/target/release/rdbmsd" ]; then
    echo "Building release binaries..."
    cd "$REPO_ROOT"
    cargo build --release -p db --features tcp-server
    cd "$SCRIPT_DIR"
else
    echo "Release binaries already exist."
fi

# Step 2: Setup users and groups
echo ""
echo "Step 2: Setting up users and groups..."
chmod +x "$SCRIPT_DIR/setup-users.sh"
"$SCRIPT_DIR/setup-users.sh"

# Step 3: Install binary
echo ""
echo "Step 3: Installing binary..."
mkdir -p "$BIN_DIR"
cp "$REPO_ROOT/target/release/rdbmsd" "$BIN_DIR/"
chmod 755 "$BIN_DIR/rdbmsd"
echo "Installed to: $BIN_DIR/rdbmsd"

# Step 4: Create directories
echo ""
echo "Step 4: Creating directories..."
mkdir -p /var/lib/rdbms
mkdir -p /run/rdbms
mkdir -p /var/log/rdbms
mkdir -p /etc/rdbms
chmod 755 /var/lib/rdbms /run/rdbms /var/log/rdbms /etc/rdbms

# Step 5: Install systemd unit
echo ""
echo "Step 5: Installing systemd unit..."
cp "$SCRIPT_DIR/rdbms.service" "$SYSTEMD_DIR/"
chmod 644 "$SYSTEMD_DIR/rdbms.service"
systemctl daemon-reload
echo "Systemd unit installed to: $SYSTEMD_DIR/rdbms.service"

# Step 6: Set permissions
echo ""
echo "Step 6: Setting permissions..."
chown -R rdbms:rdbms /var/lib/rdbms /run/rdbms /var/log/rdbms 2>/dev/null || true
echo "Permissions set."

echo ""
echo "=== Installation Complete ==="
echo ""
echo "To start the service:"
echo "  sudo systemctl start rdbms"
echo ""
echo "To enable on boot:"
echo "  sudo systemctl enable rdbms"
echo ""
echo "To check status:"
echo "  systemctl status rdbms"
echo ""
echo "To view logs:"
echo "  journalctl -u rdbms -f"
echo ""
echo "To connect:"
echo "  rdbmsd --db /var/lib/rdbms/database.db --listen 0.0.0.0:5432"
