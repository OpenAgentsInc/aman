#!/bin/bash
#
# Common utilities for Aman scripts.
# Source this file at the start of other scripts.
#
# Usage: source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"
#

# Resolve project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Load .env file if it exists
load_env() {
    local env_file="${1:-$PROJECT_ROOT/.env}"
    if [ -f "$env_file" ]; then
        # Export variables from .env file, ignoring comments and empty lines
        set -a
        # shellcheck disable=SC1090
        source "$env_file"
        set +a
    fi
}

# Load .env by default
load_env
