#!/bin/bash
#
# Register or re-register a Signal account.
#
# Usage:
#   ./scripts/register-signal.sh +1234567890           # New registration
#   ./scripts/register-signal.sh +1234567890 --voice   # Request voice call instead of SMS
#   ./scripts/register-signal.sh +1234567890 --captcha # If captcha required
#
# After running, you'll receive a verification code via SMS (or voice call).
# Then run:
#   ./scripts/signal-cli.sh -a +1234567890 verify <CODE>
#
# Environment variables (can be set in .env):
#   SIGNAL_CLI_JAR  - Path to signal-cli.jar (default: build/signal-cli.jar)
#

set -euo pipefail

# Load common utilities and .env
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

SIGNAL_CLI_JAR="${SIGNAL_CLI_JAR:-$PROJECT_ROOT/build/signal-cli.jar}"
ACCOUNT="${1:-}"
shift || true

if [ -z "$ACCOUNT" ]; then
    echo "Usage: $0 <phone_number> [--voice] [--captcha]" >&2
    echo "" >&2
    echo "Examples:" >&2
    echo "  $0 +1234567890              # SMS verification" >&2
    echo "  $0 +1234567890 --voice      # Voice call verification" >&2
    echo "  $0 +1234567890 --captcha    # Opens captcha URL first" >&2
    exit 1
fi

if [ ! -f "$SIGNAL_CLI_JAR" ]; then
    echo "Error: signal-cli.jar not found at $SIGNAL_CLI_JAR" >&2
    echo "Run: ./scripts/build-signal-cli.sh" >&2
    exit 1
fi

# Check for captcha flag
CAPTCHA=""
VOICE=""
for arg in "$@"; do
    case "$arg" in
        --captcha)
            echo "=== Captcha Required ==="
            echo "1. Open this URL in your browser:"
            echo "   https://signalcaptchas.org/registration/generate.html"
            echo ""
            echo "2. Complete the captcha"
            echo ""
            echo "3. Copy the signalcaptcha:// URL from the result"
            echo ""
            read -p "4. Paste the captcha token here: " CAPTCHA
            echo ""
            ;;
        --voice)
            VOICE="--voice"
            ;;
    esac
done

echo "=== Registering $ACCOUNT ==="
echo ""

# Build command
CMD="java -jar $SIGNAL_CLI_JAR -a $ACCOUNT register"
if [ -n "$VOICE" ]; then
    CMD="$CMD --voice"
fi
if [ -n "$CAPTCHA" ]; then
    CMD="$CMD --captcha $CAPTCHA"
fi

echo "Running: $CMD"
echo ""

eval $CMD

echo ""
echo "=== Next Steps ==="
echo "1. You should receive a verification code via ${VOICE:+voice call}${VOICE:-SMS}"
echo ""
echo "2. Run this command with your code:"
echo "   ./scripts/signal-cli.sh -a $ACCOUNT verify <CODE>"
echo ""
echo "3. Then start the daemon:"
echo "   ./scripts/run-signal-daemon.sh $ACCOUNT"
