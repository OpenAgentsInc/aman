#!/bin/bash
#
# Install Aman bot as a systemd service.
#
# The bot spawns and manages the signal-cli daemon internally,
# so only one service is needed.
#
# Usage:
#   sudo ./scripts/install-systemd.sh           # Install service
#   sudo ./scripts/install-systemd.sh --uninstall  # Remove service
#   ./scripts/install-systemd.sh --status       # Show status
#
# Prerequisites:
#   - Build signal-cli: ./scripts/build-signal-cli.sh
#   - Build bot: cargo build --release -p agent-brain
#   - Set up Signal account: ./scripts/link-device.sh
#   - Configure .env file with required variables
#
# Required environment variables (in .env):
#   AMAN_NUMBER        - Signal phone number for the bot
#
# Optional environment variables:
#   SIGNAL_CLI_JAR     - Path to signal-cli.jar (default: build/signal-cli.jar)
#   HTTP_ADDR          - Daemon HTTP address (default: 127.0.0.1:8080)
#   RUST_LOG           - Log level (default: info)
#

set -euo pipefail

# Load common utilities and .env
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

# Configuration
SERVICE_NAME="aman-bot"
SERVICE_USER="${SERVICE_USER:-$USER}"
SERVICE_GROUP="${SERVICE_GROUP:-$USER}"
INSTALL_DIR="${INSTALL_DIR:-$PROJECT_ROOT}"
SIGNAL_CLI_JAR="${SIGNAL_CLI_JAR:-$PROJECT_ROOT/build/signal-cli.jar}"
BOT_BINARY="${BOT_BINARY:-$PROJECT_ROOT/target/release/agent_brain_bot}"
RUST_LOG="${RUST_LOG:-info}"
SYSTEMD_DIR="/etc/systemd/system"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info() { echo -e "${GREEN}[INFO]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

check_sudo() {
    if [ "$EUID" -ne 0 ]; then
        error "This script must be run with sudo or as root."
        echo "Usage: sudo $0 $*"
        exit 1
    fi
}

validate_env() {
    local errors=0

    if [ -z "${AMAN_NUMBER:-}" ]; then
        error "AMAN_NUMBER not set. Add it to .env file."
        errors=$((errors + 1))
    fi

    if [ ! -f "$SIGNAL_CLI_JAR" ]; then
        error "signal-cli.jar not found at $SIGNAL_CLI_JAR"
        echo "  Run: ./scripts/build-signal-cli.sh"
        errors=$((errors + 1))
    fi

    if [ ! -f "$BOT_BINARY" ]; then
        error "Bot binary not found at $BOT_BINARY"
        echo "  Run: cargo build --release -p agent-brain"
        errors=$((errors + 1))
    fi

    if [ $errors -gt 0 ]; then
        exit 1
    fi
}

create_service() {
    info "Creating $SERVICE_NAME.service..."

    # Get the home directory for the service user
    local user_home
    user_home=$(getent passwd "$SERVICE_USER" | cut -d: -f6)

    cat > "$SYSTEMD_DIR/$SERVICE_NAME.service" << EOF
[Unit]
Description=Aman AI Signal Bot
Documentation=https://github.com/anthropics/aman
After=network.target

[Service]
Type=simple
User=$SERVICE_USER
Group=$SERVICE_GROUP
WorkingDirectory=$INSTALL_DIR

# Load environment from .env
EnvironmentFile=-$INSTALL_DIR/.env

# Override/set key variables
Environment="RUST_LOG=$RUST_LOG"
Environment="SIGNAL_CLI_JAR=$SIGNAL_CLI_JAR"

# Run the bot
ExecStart=$BOT_BINARY

# Restart policy
Restart=always
RestartSec=10

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
ReadWritePaths=$user_home/.local/share/signal-cli
ReadWritePaths=$INSTALL_DIR

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=$SERVICE_NAME

[Install]
WantedBy=multi-user.target
EOF

    info "Created $SYSTEMD_DIR/$SERVICE_NAME.service"
}

install_service() {
    create_service

    info "Reloading systemd daemon..."
    systemctl daemon-reload

    info "Enabling $SERVICE_NAME..."
    systemctl enable "$SERVICE_NAME"

    info "Starting $SERVICE_NAME..."
    systemctl start "$SERVICE_NAME"

    sleep 2

    if systemctl is-active --quiet "$SERVICE_NAME"; then
        info "$SERVICE_NAME is running"
    else
        warn "$SERVICE_NAME may have failed to start."
        echo "Check logs: journalctl -u $SERVICE_NAME -f"
    fi
}

uninstall_service() {
    info "Uninstalling $SERVICE_NAME..."

    if systemctl is-active --quiet "$SERVICE_NAME" 2>/dev/null; then
        info "Stopping $SERVICE_NAME..."
        systemctl stop "$SERVICE_NAME" || true
    fi

    if systemctl is-enabled --quiet "$SERVICE_NAME" 2>/dev/null; then
        info "Disabling $SERVICE_NAME..."
        systemctl disable "$SERVICE_NAME" || true
    fi

    if [ -f "$SYSTEMD_DIR/$SERVICE_NAME.service" ]; then
        info "Removing service file..."
        rm -f "$SYSTEMD_DIR/$SERVICE_NAME.service"
    fi

    systemctl daemon-reload
    info "Service removed."
}

show_status() {
    echo ""
    echo "=== Service Status ==="
    echo ""

    if systemctl is-active --quiet "$SERVICE_NAME" 2>/dev/null; then
        echo -e "$SERVICE_NAME: ${GREEN}running${NC}"
        echo ""
        systemctl status "$SERVICE_NAME" --no-pager -l | head -15
    else
        echo -e "$SERVICE_NAME: ${RED}stopped${NC}"
    fi

    echo ""
    echo "=== Commands ==="
    echo ""
    echo "View logs:     journalctl -u $SERVICE_NAME -f"
    echo "Restart:       sudo systemctl restart $SERVICE_NAME"
    echo "Stop:          sudo systemctl stop $SERVICE_NAME"
    echo "Start:         sudo systemctl start $SERVICE_NAME"
    echo ""
}

main() {
    case "${1:-}" in
        --uninstall)
            check_sudo
            uninstall_service
            ;;
        --status)
            show_status
            ;;
        --help|-h)
            echo "Usage: sudo $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  (none)       Install the service"
            echo "  --uninstall  Remove the service"
            echo "  --status     Show service status"
            echo "  --help       Show this help"
            exit 0
            ;;
        "")
            check_sudo
            validate_env
            install_service
            show_status
            info "Installation complete!"
            ;;
        *)
            error "Unknown option: $1"
            echo "Use --help for usage."
            exit 1
            ;;
    esac
}

main "$@"
