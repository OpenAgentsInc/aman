#!/bin/bash
#
# Development script for running the Aman bot.
#
# This script:
#   1. Builds the aman_bot binary and installs it to bin/
#   2. Connects to an existing signal daemon (or starts one)
#   3. Runs the bot to process incoming Signal messages
#
# Usage:
#   ./scripts/dev.sh              # Run bot (builds only if binary missing)
#   ./scripts/dev.sh --build      # Force rebuild before running
#   ./scripts/dev.sh --daemon     # Also start signal daemon in background
#
# Required environment variables (set in .env):
#   MAPLE_API_KEY   - OpenSecret API key
#   GROK_API_KEY    - xAI API key for real-time search
#
# Optional environment variables:
#   AMAN_NUMBER       - Signal phone number (needed if spawning daemon)
#   SIGNAL_DAEMON_URL - URL of running daemon (default: http://localhost:8080)
#   RUST_LOG          - Log level (default: info)
#

set -euo pipefail

# Load common utilities and .env
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

# Configuration
FORCE_BUILD=false
START_DAEMON=false
BIN_DIR="$PROJECT_ROOT/bin"
BINARY_NAME="aman_bot"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --build)
            FORCE_BUILD=true
            shift
            ;;
        --daemon)
            START_DAEMON=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --build      Force rebuild even if binary exists"
            echo "  --daemon     Start signal daemon in background first"
            echo "  -h, --help   Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

# Check required environment variables
check_env() {
    local missing=()

    if [ -z "${MAPLE_API_KEY:-}" ]; then
        missing+=("MAPLE_API_KEY")
    fi

    if [ -z "${GROK_API_KEY:-}" ]; then
        missing+=("GROK_API_KEY")
    fi

    if [ ${#missing[@]} -gt 0 ]; then
        echo "Error: Missing required environment variables:" >&2
        for var in "${missing[@]}"; do
            echo "  - $var" >&2
        done
        echo "" >&2
        echo "Set these in your .env file or environment." >&2
        exit 1
    fi
}

# Build the bot
build_bot() {
    echo "Building aman_bot..."

    # Build in release mode for better performance
    cargo build --release --example aman_bot \
        --manifest-path "$PROJECT_ROOT/crates/maple-brain/Cargo.toml"

    # Ensure bin directory exists
    mkdir -p "$BIN_DIR"

    # The binary is placed in the workspace root's target directory
    local binary_path="$PROJECT_ROOT/target/release/examples/$BINARY_NAME"

    if [ -f "$binary_path" ]; then
        cp "$binary_path" "$BIN_DIR/"
        echo "Installed: $BIN_DIR/$BINARY_NAME"
    else
        echo "Error: Built binary not found at $binary_path" >&2
        exit 1
    fi
}

# Check if daemon is running
daemon_running() {
    local url="${SIGNAL_DAEMON_URL:-http://localhost:8080}"
    curl -s -o /dev/null -w '' "$url/api/v1/check" 2>/dev/null
}

# Start daemon in background
start_daemon() {
    if [ -z "${AMAN_NUMBER:-}" ]; then
        echo "Error: AMAN_NUMBER required to start daemon" >&2
        exit 1
    fi

    echo "Starting signal daemon in background..."
    "$PROJECT_ROOT/scripts/run-signal-daemon.sh" &
    DAEMON_PID=$!

    # Wait for daemon to be ready
    echo "Waiting for daemon to start..."
    for i in {1..30}; do
        if daemon_running; then
            echo "Daemon started (PID: $DAEMON_PID)"
            return 0
        fi
        sleep 1
    done

    echo "Error: Daemon failed to start within 30 seconds" >&2
    kill $DAEMON_PID 2>/dev/null || true
    exit 1
}

# Cleanup on exit
cleanup() {
    if [ -n "${DAEMON_PID:-}" ]; then
        echo "Stopping daemon (PID: $DAEMON_PID)..."
        kill $DAEMON_PID 2>/dev/null || true
    fi
}
trap cleanup EXIT

# Main
main() {
    echo "=== Aman Bot Development ==="
    echo ""

    # Check environment
    check_env

    # Build if binary doesn't exist or --build was passed
    if $FORCE_BUILD || [ ! -x "$BIN_DIR/$BINARY_NAME" ]; then
        build_bot
        echo ""
    else
        echo "Using existing binary: $BIN_DIR/$BINARY_NAME"
        echo "(use --build to rebuild)"
        echo ""
    fi

    # Start daemon if requested
    if $START_DAEMON; then
        if daemon_running; then
            echo "Signal daemon already running."
        else
            start_daemon
        fi
        echo ""
    fi

    # Check daemon is available
    if ! daemon_running; then
        echo "Warning: Signal daemon not detected at ${SIGNAL_DAEMON_URL:-http://localhost:8080}"
        echo "The bot will attempt to spawn a daemon if AMAN_NUMBER is set."
        echo ""
    fi

    # Set default log level if not set
    export RUST_LOG="${RUST_LOG:-info}"

    # Run the bot
    echo "Starting aman_bot..."
    echo "  Log level: $RUST_LOG"
    echo "  Binary: $BIN_DIR/$BINARY_NAME"
    echo ""
    echo "Press Ctrl+C to stop."
    echo "---"

    exec "$BIN_DIR/$BINARY_NAME"
}

main "$@"
