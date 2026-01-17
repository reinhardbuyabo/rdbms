#!/bin/bash
# RDBMS User and Group Setup Script
# Creates dedicated system user and group for RDBMS service

set -e

USER_NAME="rdbms"
USER_COMMENT="RDBMS Database Server"
USER_HOME="/var/lib/rdbms"
USER_SHELL="/sbin/nologin"

# Allow override via environment variables
USER_GID="${USER_GID:-}"
USER_UID="${USER_UID:-}"

echo "=== RDBMS User and Group Setup ==="

# Check if running as root
if [ "$(id -u)" -ne 0 ]; then
    echo "Error: This script must be run as root (use sudo)"
    exit 1
fi

# Function to find an available GID
find_available_gid() {
    local gid=999
    while getent group "$gid" > /dev/null 2>&1; do
        gid=$((gid + 1))
    done
    echo "$gid"
}

# Function to find an available UID
find_available_uid() {
    local uid=999
    while getent passwd "$uid" > /dev/null 2>&1; do
        uid=$((uid + 1))
    done
    echo "$uid"
}

# Determine GID
if [ -z "$USER_GID" ]; then
    USER_GID=$(find_available_gid)
fi

# Create group if it doesn't exist
if ! getent group "$USER_NAME" > /dev/null 2>&1; then
    echo "Creating group: $USER_NAME"
    groupadd --system \
        --gid "$USER_GID" \
        "$USER_NAME"
else
    echo "Group already exists: $USER_NAME"
    USER_GID=$(getent group "$USER_NAME" | cut -d: -f3)
fi

# Determine UID
if [ -z "$USER_UID" ]; then
    USER_UID=$(find_available_uid)
fi

# Create user if it doesn't exist
if ! getent passwd "$USER_NAME" > /dev/null 2>&1; then
    echo "Creating user: $USER_NAME"
    useradd --system \
        --gid "$USER_NAME" \
        --uid "$USER_UID" \
        --home-dir "$USER_HOME" \
        --shell "$USER_SHELL" \
        --comment "$USER_COMMENT" \
        "$USER_NAME"
else
    echo "User already exists: $USER_NAME"
    USER_UID=$(getent passwd "$USER_NAME" | cut -d: -f3)
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
echo "User: $USER_NAME (UID: $USER_UID)"
echo "Group: $USER_NAME (GID: $USER_GID)"
echo "Home: $USER_HOME"
echo ""
echo "To install the service:"
echo "  sudo ./install.sh"
echo ""
echo "To enable socket activation:"
echo "  sudo systemctl enable rdbms.socket"
