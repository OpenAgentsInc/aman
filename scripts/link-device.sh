#!/bin/bash
#
# Link signal-cli as a secondary device to an existing Signal account.
#
# This generates a QR code that you scan with your primary Signal app
# (Settings > Linked Devices > Link New Device).
#
# Usage:
#   ./scripts/link-device.sh                    # Uses default device name "aman-bot"
#   ./scripts/link-device.sh "My Server"        # Custom device name
#
# Environment variables (can be set in .env):
#   SIGNAL_CLI_JAR  - Path to signal-cli.jar (default: build/signal-cli.jar)
#

set -euo pipefail

# Load common utilities and .env
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

SIGNAL_CLI_JAR="${SIGNAL_CLI_JAR:-$PROJECT_ROOT/build/signal-cli.jar}"
DEVICE_NAME="${1:-aman-bot}"

if [ ! -f "$SIGNAL_CLI_JAR" ]; then
    echo "Error: signal-cli.jar not found at $SIGNAL_CLI_JAR" >&2
    echo "Run: ./scripts/build-signal-cli.sh" >&2
    exit 1
fi

echo "=== Linking Device: $DEVICE_NAME ==="
echo ""
echo "A QR code URI will be generated below."
echo ""
echo "To link:"
echo "  1. Open Signal on your primary device (phone)"
echo "  2. Go to Settings > Linked Devices"
echo "  3. Tap 'Link New Device'"
echo "  4. Scan the QR code (or use the URI with a QR generator)"
echo ""
echo "Waiting for link request..."
echo ""

java -jar "$SIGNAL_CLI_JAR" link -n "$DEVICE_NAME"

echo ""
echo "=== Device Linked Successfully ==="
echo ""
echo "Your linked account number should now be available."
echo "You can verify with:"
echo "  ./scripts/signal-cli.sh -a <YOUR_NUMBER> receive"
echo ""
echo "Or start the daemon:"
echo "  ./scripts/run-signal-daemon.sh <YOUR_NUMBER>"
