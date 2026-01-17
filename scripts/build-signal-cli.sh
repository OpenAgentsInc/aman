#!/bin/bash
#
# Build signal-cli fat JAR and store in the build directory.
#
# Usage: ./scripts/build-signal-cli.sh
#
# Output: build/signal-cli.jar
#

set -euo pipefail

# Load common utilities and .env
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

SIGNAL_CLI_DIR="$PROJECT_ROOT/repos/signal-cli"
BUILD_DIR="$PROJECT_ROOT/build"
OUTPUT_JAR="$BUILD_DIR/signal-cli.jar"

# Check Java 21+
check_java() {
    if ! command -v java &> /dev/null; then
        echo "Error: Java is not installed. Please install JDK 21 or higher." >&2
        exit 1
    fi

    JAVA_VERSION=$(java -version 2>&1 | head -1 | cut -d'"' -f2 | cut -d'.' -f1)
    if [ "$JAVA_VERSION" -lt 21 ]; then
        echo "Error: Java 21+ required. Found version $JAVA_VERSION." >&2
        exit 1
    fi
}

# Check signal-cli submodule
check_submodule() {
    if [ ! -f "$SIGNAL_CLI_DIR/gradlew" ]; then
        echo "Error: signal-cli submodule not found. Run: git submodule update --init" >&2
        exit 1
    fi
}

# Build and copy
build() {
    echo "Building signal-cli..."
    cd "$SIGNAL_CLI_DIR"
    ./gradlew fatJar --quiet

    FAT_JAR=$(find "$SIGNAL_CLI_DIR/build/libs" -name "signal-cli-fat-*.jar" -type f | head -1)
    if [ -z "$FAT_JAR" ]; then
        echo "Error: Fat JAR not found" >&2
        exit 1
    fi

    mkdir -p "$BUILD_DIR"
    cp "$FAT_JAR" "$OUTPUT_JAR"
    echo "Built: $OUTPUT_JAR"
}

check_java
check_submodule
build
