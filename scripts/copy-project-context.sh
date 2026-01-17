#!/bin/bash
#
# Copy markdown docs, Cargo.toml files, and all source files under src/ to clipboard.
#
# Usage: ./scripts/copy-project-context.sh
#

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

TEMP_FILE=""

collect_files() {
    if command -v rg >/dev/null 2>&1; then
        rg --files \
            -g '*.md' \
            -g 'Cargo.toml' \
            -g 'src/**' \
            -g '!**/.git/**' \
            -g '!**/target/**' \
            -g '!**/node_modules/**' \
            -g '!**/.venv/**' \
            -g '!**/.idea/**' \
            -g '!**/.vscode/**' \
            -g '!**/.direnv/**' \
            -g '!**/.next/**' \
            -g '!**/dist/**' \
            -g '!**/build/**' \
            -g '!**/out/**' \
            -g '!**/.cache/**' \
            -g '!**/.pytest_cache/**' \
            -g '!**/.ruff_cache/**' \
            -g '!**/.tox/**' \
            -g '!**/coverage/**'
    else
        find "$PROJECT_ROOT" \
            \( -path "$PROJECT_ROOT/.git" \
            -o -path "$PROJECT_ROOT/target" \
            -o -path "$PROJECT_ROOT/node_modules" \
            -o -path "$PROJECT_ROOT/.venv" \
            -o -path "$PROJECT_ROOT/.idea" \
            -o -path "$PROJECT_ROOT/.vscode" \
            -o -path "$PROJECT_ROOT/.direnv" \
            -o -path "$PROJECT_ROOT/.next" \
            -o -path "$PROJECT_ROOT/dist" \
            -o -path "$PROJECT_ROOT/build" \
            -o -path "$PROJECT_ROOT/out" \
            -o -path "$PROJECT_ROOT/.cache" \
            -o -path "$PROJECT_ROOT/.pytest_cache" \
            -o -path "$PROJECT_ROOT/.ruff_cache" \
            -o -path "$PROJECT_ROOT/.tox" \
            -o -path "$PROJECT_ROOT/coverage" \) -prune -o \
            -type f \( -name '*.md' -o -name 'Cargo.toml' -o -path '*/src/*' \) -print \
            | sed "s|^$PROJECT_ROOT/||"
    fi
}

copy_to_clipboard() {
    local input_file="$1"

    if command -v pbcopy >/dev/null 2>&1; then
        cat "$input_file" | pbcopy
        return 0
    fi

    if command -v xclip >/dev/null 2>&1; then
        cat "$input_file" | xclip -selection clipboard
        return 0
    fi

    if command -v xsel >/dev/null 2>&1; then
        cat "$input_file" | xsel --clipboard --input
        return 0
    fi

    echo "Error: No clipboard tool found (pbcopy/xclip/xsel)." >&2
    return 1
}

main() {
    local -a files
    local tmp_file
    local file_count

    cd "$PROJECT_ROOT"

    while IFS= read -r line; do
        [ -n "$line" ] && files+=("$line")
    done < <(collect_files | sort -u)
    if [ "${#files[@]}" -eq 0 ]; then
        echo "Error: No matching files found." >&2
        exit 1
    fi

    tmp_file="$(mktemp "${TMPDIR:-/tmp}/aman-context.XXXXXX")"
    TEMP_FILE="$tmp_file"
    trap 'rm -f "${TEMP_FILE:-}"' EXIT

    for relpath in "${files[@]}"; do
        printf -- "----- %s -----\n" "$relpath" >> "$tmp_file"
        cat "$relpath" >> "$tmp_file"
        printf "\n" >> "$tmp_file"
    done

    copy_to_clipboard "$tmp_file"

    file_count="${#files[@]}"
    echo "Copied $file_count files to clipboard."
}

main "$@"
