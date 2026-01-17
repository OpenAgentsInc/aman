#!/bin/bash
#
# Unlink this device and delete all local Signal data.
#
# This removes signal-cli's local account data, effectively unlinking this
# device from your Signal account. Use this to:
#   - Reset signal-cli state before re-linking
#   - Clean up after testing
#   - Remove a linked or registered account from this machine
#
# NOTE: This does NOT unregister the account from Signal servers.
#       Other devices (your phone, other linked devices) are unaffected.
#       To fully unregister an account, use: signal-cli unregister
#
# Usage:
#   ./scripts/unlink-device.sh              # Interactive confirmation
#   ./scripts/unlink-device.sh --force      # Skip confirmation (for scripts)
#
# Environment variables (can be set in .env):
#   SIGNAL_CLI_JAR  - Path to signal-cli.jar (default: build/signal-cli.jar)
#

set -euo pipefail

# Load common utilities and .env
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

SIGNAL_CLI_JAR="${SIGNAL_CLI_JAR:-$PROJECT_ROOT/build/signal-cli.jar}"
SIGNAL_DATA_DIR="${HOME}/.local/share/signal-cli/data"
FORCE="${1:-}"

# Check if data directory exists
if [ ! -d "$SIGNAL_DATA_DIR" ]; then
    echo "No signal-cli data found at $SIGNAL_DATA_DIR"
    echo "Nothing to unlink."
    exit 0
fi

# List accounts that will be removed
echo "=== Unlink Device ==="
echo ""
echo "This will delete ALL local Signal data from this machine."
echo ""
echo "Data directory: $SIGNAL_DATA_DIR"
echo ""
echo "Accounts found:"
for account_dir in "$SIGNAL_DATA_DIR"/+*/ "$SIGNAL_DATA_DIR"/*@*/ 2>/dev/null; do
    if [ -d "$account_dir" ]; then
        account_name=$(basename "$account_dir")
        echo "  - $account_name"
    fi
done
echo ""
echo "WARNING: This action cannot be undone!"
echo "         You will need to re-link or re-register to use signal-cli again."
echo ""

# Confirm unless --force
if [ "$FORCE" != "--force" ]; then
    read -p "Are you sure you want to delete all local Signal data? [y/N] " confirm
    if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
        echo "Aborted."
        exit 1
    fi
    echo ""
fi

# Try to cleanly delete via signal-cli first (if JAR exists)
if [ -f "$SIGNAL_CLI_JAR" ]; then
    echo "Cleaning up accounts via signal-cli..."
    for account_dir in "$SIGNAL_DATA_DIR"/+*/ "$SIGNAL_DATA_DIR"/*@*/ 2>/dev/null; do
        if [ -d "$account_dir" ]; then
            account_name=$(basename "$account_dir")
            echo "  Deleting local data for $account_name..."
            java -jar "$SIGNAL_CLI_JAR" -a "$account_name" deleteLocalAccountData 2>/dev/null || true
        fi
    done
    echo ""
fi

# Remove the data directory
echo "Removing data directory..."
rm -rf "$SIGNAL_DATA_DIR"

echo ""
echo "=== Unlink Complete ==="
echo ""
echo "All local Signal data has been removed from this machine."
echo ""
echo "To use signal-cli again, either:"
echo "  - Link to your phone:  ./scripts/link-device.sh"
echo "  - Register new number: ./scripts/register-signal.sh +1234567890"
