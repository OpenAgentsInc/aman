#!/bin/bash
#
# Link signal-cli as a secondary device to an existing Signal account.
#
# This is the RECOMMENDED approach for development because:
#   - Multiple machines can link to the same account
#   - Your phone remains the primary device
#   - Easy to unlink/relink without losing the account
#
# How it works:
#   1. This script generates a QR code in the terminal
#   2. You scan the QR with your phone's Signal app
#   3. Your phone authorizes this machine as a secondary device
#   4. This machine can now send/receive messages on your account
#
# Usage:
#   ./scripts/link-device.sh <device-name>
#
# Examples:
#   ./scripts/link-device.sh "My Laptop"
#   ./scripts/link-device.sh "Dev Server"
#   ./scripts/link-device.sh "aman-prod"
#
# Requirements:
#   qrencode - Install with: sudo apt install qrencode (Debian/Ubuntu)
#                            brew install qrencode (macOS)
#
# Environment variables (can be set in .env):
#   SIGNAL_CLI_JAR  - Path to signal-cli.jar (default: build/signal-cli.jar)
#

set -euo pipefail

# Load common utilities and .env
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

SIGNAL_CLI_JAR="${SIGNAL_CLI_JAR:-$PROJECT_ROOT/build/signal-cli.jar}"
DEVICE_NAME="${1:-}"

if [ -z "$DEVICE_NAME" ]; then
    echo "Usage: $0 <device-name>" >&2
    echo "" >&2
    echo "Examples:" >&2
    echo "  $0 \"My Laptop\"" >&2
    echo "  $0 \"Dev Server\"" >&2
    echo "  $0 \"aman-prod\"" >&2
    exit 1
fi

if [ ! -f "$SIGNAL_CLI_JAR" ]; then
    echo "Error: signal-cli.jar not found at $SIGNAL_CLI_JAR" >&2
    echo "Run: ./scripts/build-signal-cli.sh" >&2
    exit 1
fi

# Check for qrencode
if ! command -v qrencode &> /dev/null; then
    echo "Error: qrencode is not installed" >&2
    echo "" >&2
    echo "Install it with:" >&2
    echo "  Ubuntu/Debian: sudo apt install qrencode" >&2
    echo "  macOS:         brew install qrencode" >&2
    echo "  Fedora:        sudo dnf install qrencode" >&2
    echo "  Arch:          sudo pacman -S qrencode" >&2
    exit 1
fi

echo "=== Link Device: $DEVICE_NAME ==="
echo ""
echo "This will link this machine as a secondary device to your Signal account."
echo ""
echo "STEPS:"
echo "  1. A QR code will appear below"
echo "  2. Open Signal on your phone"
echo "  3. Go to: Settings > Linked Devices > Link New Device"
echo "  4. Scan the QR code"
echo "  5. Approve the link on your phone"
echo ""
echo "Generating link..."
echo ""

# Run signal-cli link and process its output
# When we see a tsdevice:// URI, display it as a QR code
java -jar "$SIGNAL_CLI_JAR" link -n "$DEVICE_NAME" 2>&1 | while IFS= read -r line; do
    if [[ "$line" == tsdevice://* ]] || [[ "$line" == sgnl://* ]]; then
        echo "=== Scan this QR code with your phone ==="
        echo ""
        echo "$line" | qrencode -t UTF8
        echo ""
        echo "URI: $line"
        echo ""
        echo "Waiting for you to scan and approve on your phone..."
    else
        echo "$line"
    fi
done

echo ""
echo "=== Link Complete ==="
echo ""
echo "If linking succeeded, this device is now connected to your Signal account."
echo ""
echo "To find your account number, check your phone or run:"
echo "  ls ~/.local/share/signal-cli/data/"
echo ""
echo "Then start the daemon with:"
echo "  ./scripts/run-signal-daemon.sh +1234567890"
echo "  # Or set AMAN_NUMBER in .env and run:"
echo "  ./scripts/run-signal-daemon.sh"
