#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
LOG_FILE="${TMPDIR:-/tmp}/beforepaste-install.log"
BUILD_DIR="${PROJECT_DIR}/target"

GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BOLD='\033[1m'
NC='\033[0m'

log() { echo "[$(date '+%H:%M:%S')] $*" >> "$LOG_FILE"; }

print_banner() {
    printf "\n${CYAN}${BOLD}  BeforePaste source installer${NC}\n"
    printf "${YELLOW}  Log: ${LOG_FILE}${NC}\n\n"
}

step() { printf "\n${BOLD}[${CYAN}${1}${NC}${BOLD}]${NC} ${2}\n"; }

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        printf "  ${RED}[FAIL]${NC} Missing required command: %s\n" "$1" >&2
        return 1
    fi
}

check_deps() {
    require_cmd cargo
    require_cmd rustc
    case "$(uname -s)" in
        Linux*)
            printf "  ${YELLOW}[!]${NC} Linux builds may need X11, xkbcommon, DBus, GTK/WebKit packages.\n"
            printf "      See README.md and desktop/README.md for distro-specific notes.\n"
            ;;
        Darwin*)
            printf "  ${GREEN}[OK]${NC} macOS Rust toolchain detected.\n"
            ;;
        MINGW*|MSYS*|CYGWIN*)
            printf "  ${GREEN}[OK]${NC} Windows Rust toolchain detected.\n"
            ;;
    esac
}

build_release() {
    log "Building beforepaste release..."
    cargo build --release >> "$LOG_FILE" 2>&1
    local bin="$BUILD_DIR/release/beforepaste"
    if [ ! -f "$bin" ]; then
        printf "  ${RED}[FAIL]${NC} Build failed - check ${LOG_FILE}\n" >&2
        return 1
    fi
    printf "  ${GREEN}[OK]${NC} Built ${bin}\n"
}

install_binary() {
    local bin="$BUILD_DIR/release/beforepaste"
    local dest="/usr/local/bin/beforepaste"

    if command -v sudo >/dev/null 2>&1 && sudo cp "$bin" "$dest" 2>>"$LOG_FILE"; then
        sudo chmod +x "$dest" 2>>"$LOG_FILE" || true
        printf "  ${GREEN}[OK]${NC} Installed to ${dest}\n"
        return 0
    fi

    mkdir -p "$HOME/.local/bin"
    cp "$bin" "$HOME/.local/bin/beforepaste"
    chmod +x "$HOME/.local/bin/beforepaste"
    printf "  ${GREEN}[OK]${NC} Installed to ${HOME}/.local/bin/beforepaste\n"
    case ":$PATH:" in
        *":$HOME/.local/bin:"*) ;;
        *) printf "  ${YELLOW}[!]${NC} Add ~/.local/bin to your PATH if needed.\n" ;;
    esac
}

print_summary() {
    printf "\n${GREEN}${BOLD}BeforePaste CLI installed.${NC}\n\n"
    printf "  ${CYAN}beforepaste redact${NC}   - Redact stdin to stdout\n"
    printf "  ${CYAN}beforepaste trigger${NC}  - One-shot protected clipboard workflow\n"
    printf "  ${CYAN}beforepaste menu${NC}     - CLI/TUI settings for advanced users\n\n"
    printf "  For tray-based paste protection, install the desktop app from GitHub Releases\n"
    printf "  or build it with: ${CYAN}cd desktop && npm ci && npm run build:no-bundle${NC}\n"
    printf "\n  ${YELLOW}Install log: ${LOG_FILE}${NC}\n\n"
}

main() {
    print_banner
    : > "$LOG_FILE"
    cd "$PROJECT_DIR"

    step "1" "Checking source build requirements"
    check_deps

    step "2" "Building BeforePaste CLI"
    build_release

    step "3" "Installing CLI binary"
    install_binary

    print_summary
}

main "$@"
