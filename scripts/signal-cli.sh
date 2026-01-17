#!/bin/bash
#
# General signal-cli wrapper - passes all arguments to the JAR.
#
# Usage:
#   ./scripts/signal-cli.sh --help
#   ./scripts/signal-cli.sh -a +1234567890 register
#   ./scripts/signal-cli.sh -a +1234567890 verify 123456
#   ./scripts/signal-cli.sh -a +1234567890 daemon --http=127.0.0.1:8080
#
# Environment variables (can be set in .env):
#   SIGNAL_CLI_JAR  - Path to signal-cli.jar (default: build/signal-cli.jar)
#

set -euo pipefail

# Load common utilities and .env
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

SIGNAL_CLI_JAR="${SIGNAL_CLI_JAR:-$PROJECT_ROOT/build/signal-cli.jar}"

if [ ! -f "$SIGNAL_CLI_JAR" ]; then
    echo "Error: signal-cli.jar not found at $SIGNAL_CLI_JAR" >&2
    echo "Run: ./scripts/build-signal-cli.sh" >&2
    exit 1
fi

exec java -jar "$SIGNAL_CLI_JAR" "$@"
