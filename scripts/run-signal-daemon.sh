#!/bin/bash
#
# Run signal-cli daemon for testing.
#
# Usage:
#   ./scripts/run-signal-daemon.sh                    # Uses AMAN_NUMBER from .env
#   ./scripts/run-signal-daemon.sh +1234567890        # Specify account
#   AMAN_NUMBER=+1234567890 ./scripts/run-signal-daemon.sh
#
# Environment variables (can be set in .env):
#   AMAN_NUMBER     - Signal phone number (required if not passed as argument)
#   SIGNAL_CLI_JAR  - Path to signal-cli.jar (default: build/signal-cli.jar)
#   HTTP_ADDR       - HTTP bind address (default: 127.0.0.1:8080)
#

set -euo pipefail

# Load common utilities and .env
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

# Configuration
SIGNAL_CLI_JAR="${SIGNAL_CLI_JAR:-$PROJECT_ROOT/build/signal-cli.jar}"
HTTP_ADDR="${HTTP_ADDR:-127.0.0.1:8080}"
ACCOUNT="${1:-${AMAN_NUMBER:-}}"

# Validate
if [ -z "$ACCOUNT" ]; then
    echo "Error: No account specified." >&2
    echo "Usage: $0 <phone_number>" >&2
    echo "   or: Set AMAN_NUMBER in .env file" >&2
    exit 1
fi

if [ ! -f "$SIGNAL_CLI_JAR" ]; then
    echo "Error: signal-cli.jar not found at $SIGNAL_CLI_JAR" >&2
    echo "Run: ./scripts/build-signal-cli.sh" >&2
    exit 1
fi

echo "Starting signal-cli daemon..."
echo "  Account: $ACCOUNT"
echo "  HTTP:    http://$HTTP_ADDR"
echo "  JAR:     $SIGNAL_CLI_JAR"
echo ""
echo "Endpoints:"
echo "  Health:  curl http://$HTTP_ADDR/api/v1/check"
echo "  RPC:     curl -X POST http://$HTTP_ADDR/api/v1/rpc -H 'Content-Type: application/json' -d '{\"jsonrpc\":\"2.0\",\"method\":\"version\",\"id\":1}'"
echo "  Events:  curl -N http://$HTTP_ADDR/api/v1/events"
echo ""
echo "Press Ctrl+C to stop."
echo "---"

exec java -jar "$SIGNAL_CLI_JAR" \
    --trust-new-identities=on-first-use \
    -a "$ACCOUNT" \
    daemon \
    --http="$HTTP_ADDR" \
    --send-read-receipts
