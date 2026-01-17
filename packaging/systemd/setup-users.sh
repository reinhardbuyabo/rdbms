#!/bin/bash
# RDBMS User and Group Setup Script
# Creates dedicated system user and group for RDBMS service

set -e

USER_NAME="rdbms"
USER_COMMENT="RDBMS Database Server"
USER_HOME="/var/lib/rdbms"
USER_SHELL="/sbin/nologin"

echo "=== RDBMS User and Group Setup ==="

# Check if running as root
if [ "$(id -u)" -ne 0 ]; then
    echo "Error: This script must be run as root (use sudo)"
    exit 1
fi

# Create group if it doesn't exist
if ! getent group "$USER_NAME" > /dev/null 2>&1; then
    echo "Creating group: $USER_NAME"
    groupadd --system \
        --gid 999 \
        "$USER_NAME"
else
    echo "Group already exists: $USER_NAME"
fi

# Create user if it doesn't exist
if ! getent passwd "$USER_NAME" > /dev/null 2>&1; then
    echo "Creating user: $USER_NAME"
    useradd --system \
        --gid "$USER_NAME" \
        --uid 999 \
        --home-dir "$USER_HOME" \
        --shell "$USER_SHELL" \
        --comment "$USER_COMMENT" \
        "$USER_NAME"
else
    echo "User already exists: $USER_NAME"
fi

# Create required directories with proper permissions
echo "Creating directories..."

# Main data directory
mkdir -p "$USER_HOME"
chown "$USER_NAME:$USER_NAME" "$USER_HOME"
chmod 0755 "$USER_HOME"

# State directory (for runtime state)
mkdir -p /run/rdbms
chown "$USER_NAME:$USER_NAME" /run/rdbms
chmod 0755 /run/rdbms

# Logs directory
mkdir -p /var/log/rdbms
chown "$USER_NAME:$USER_NAME" /var/log/rdbms
chmod 0755 /var/log/rdbms

# Configuration directory
mkdir -p /etc/rdbms
chown root:root /etc/rdbms
chmod 0755 /etc/rdbms

echo ""
echo "=== Setup Complete ==="
echo "User: $USER_NAME"
echo "Group: $USER_NAME"
echo "Home: $USER_HOME"
echo ""
echo "To install the service:"
echo "  sudo ./install.sh"
echo ""
echo "To enable socket activation:"
echo "  sudo systemctl enable rdbms.socket"
