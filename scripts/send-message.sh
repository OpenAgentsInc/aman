#!/bin/bash
#
# Send a test message via the signal-cli daemon (JSON-RPC).
#
# This script sends a message through the running signal-cli daemon using
# the JSON-RPC HTTP interface. The daemon must be running.
#
# Usage:
#   ./scripts/send-message.sh <recipient> <message>
#
# Recipient formats:
#   Phone number: +1234567890, "+1 (234) 567-8901"
#   UUID:         c27fb365-0c84-4cf2-8555-814bb065e448
#   Username:     username.01, alice.42
#
# Examples:
#   ./scripts/send-message.sh +1234567890 "Hello, world!"
#   ./scripts/send-message.sh c27fb365-0c84-4cf2-8555-814bb065e448 "Hello via UUID!"
#   ./scripts/send-message.sh username.01 "Hello via username!"
#
# Environment variables (can be set in .env):
#   AMAN_NUMBER  - Sender account phone number (required)
#   HTTP_ADDR    - Daemon HTTP address (default: 127.0.0.1:8080)
#

set -euo pipefail

# Load common utilities and .env
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

RECIPIENT="${1:-}"
MESSAGE="${2:-}"
HTTP_ADDR="${HTTP_ADDR:-127.0.0.1:8080}"
AMAN_NUMBER="${AMAN_NUMBER:-}"

# Validate arguments
if [ -z "$RECIPIENT" ] || [ -z "$MESSAGE" ]; then
    echo "Usage: $0 <recipient> <message>" >&2
    echo "" >&2
    echo "Recipient formats:" >&2
    echo "  Phone number: +1234567890" >&2
    echo "  UUID:         c27fb365-0c84-4cf2-8555-814bb065e448" >&2
    echo "  Username:     username.01" >&2
    echo "" >&2
    echo "Examples:" >&2
    echo "  $0 +1234567890 \"Hello, world!\"" >&2
    echo "  $0 c27fb365-0c84-4cf2-8555-814bb065e448 \"Hello via UUID!\"" >&2
    echo "  $0 username.01 \"Hello via username!\"" >&2
    exit 1
fi

# Validate AMAN_NUMBER
if [ -z "$AMAN_NUMBER" ]; then
    echo "Error: AMAN_NUMBER is not set" >&2
    echo "" >&2
    echo "Set it in .env or export it:" >&2
    echo "  export AMAN_NUMBER=+1234567890" >&2
    exit 1
fi

# Determine recipient type: phone number, UUID, or username
# UUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx (8-4-4-4-12 hex chars)
UUID_REGEX='^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$'

if [[ "$RECIPIENT" =~ $UUID_REGEX ]]; then
    # UUID - use as-is
    RECIPIENT_NORMALIZED="$RECIPIENT"
    RECIPIENT_TYPE="uuid"
elif [[ "$RECIPIENT" == +* ]] || [[ "$RECIPIENT" =~ ^[0-9\ \(\)\-]+$ ]]; then
    # Phone number - normalize by removing spaces, dashes, parentheses
    RECIPIENT_NORMALIZED=$(echo "$RECIPIENT" | tr -d ' ()-')
    # Ensure it starts with + if it doesn't already
    if [[ "$RECIPIENT_NORMALIZED" != +* ]]; then
        RECIPIENT_NORMALIZED="+$RECIPIENT_NORMALIZED"
    fi
    RECIPIENT_TYPE="phone"
else
    # Username - use as-is
    RECIPIENT_NORMALIZED="$RECIPIENT"
    RECIPIENT_TYPE="username"
fi

# Check if daemon is running
DAEMON_URL="http://${HTTP_ADDR}"
if ! curl -s -o /dev/null -w "" "${DAEMON_URL}/api/v1/check" 2>/dev/null; then
    echo "Error: signal-cli daemon is not running at ${DAEMON_URL}" >&2
    echo "" >&2
    echo "Start it with:" >&2
    echo "  ./scripts/run-signal-daemon.sh" >&2
    exit 1
fi

echo "=== Send Message ==="
echo ""
echo "From:    $AMAN_NUMBER"
echo "To:      $RECIPIENT_NORMALIZED ($RECIPIENT_TYPE)"
echo "Message: $MESSAGE"
echo ""

# Build JSON-RPC request
# Escape message for JSON (handle quotes, backslashes, newlines)
MESSAGE_ESCAPED=$(printf '%s' "$MESSAGE" | jq -Rs '.')

JSON_PAYLOAD=$(cat <<EOF
{
  "jsonrpc": "2.0",
  "method": "send",
  "params": {
    "account": "$AMAN_NUMBER",
    "recipient": ["$RECIPIENT_NORMALIZED"],
    "message": $MESSAGE_ESCAPED
  },
  "id": "send-$(date +%s)"
}
EOF
)

echo "Sending via JSON-RPC..."
echo ""

# Send the request
RESPONSE=$(curl -s -X POST "${DAEMON_URL}/api/v1/rpc" \
    -H "Content-Type: application/json" \
    -d "$JSON_PAYLOAD")

# Check for errors
if echo "$RESPONSE" | jq -e '.error' > /dev/null 2>&1; then
    ERROR_MSG=$(echo "$RESPONSE" | jq -r '.error.message // .error')
    echo "Error: $ERROR_MSG" >&2
    echo "" >&2
    echo "Full response:" >&2
    echo "$RESPONSE" | jq . 2>/dev/null || echo "$RESPONSE" >&2
    exit 1
fi

# Extract timestamp from result
TIMESTAMP=$(echo "$RESPONSE" | jq -r '.result.timestamp // empty')

if [ -n "$TIMESTAMP" ]; then
    echo "=== Message Sent ==="
    echo ""
    echo "Timestamp: $TIMESTAMP"

    # Check per-recipient results if available
    RESULTS=$(echo "$RESPONSE" | jq -r '.result.results // empty')
    if [ -n "$RESULTS" ] && [ "$RESULTS" != "null" ] && [ "$RESULTS" != "[]" ]; then
        echo ""
        echo "Recipient results:"
        echo "$RESPONSE" | jq -r '
            .result.results[] |
            "  " + (.recipientAddress.number // .recipientAddress.uuid // "unknown") + ": " +
            (if .success then "delivered" else "failed: " + (.error // "unknown error") end)
        '
    fi
else
    echo "Response:"
    echo "$RESPONSE" | jq . 2>/dev/null || echo "$RESPONSE"
fi
