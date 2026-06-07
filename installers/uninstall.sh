#!/usr/bin/env bash
set -euo pipefail

GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m'

echo ""
echo -e "${CYAN}${BOLD}  BeforePaste Uninstaller${NC}"
echo ""

run_cli_uninstall() {
    if command -v beforepaste >/dev/null 2>&1; then
        echo -e "  ${YELLOW}[~]${NC} Removing BeforePaste shortcuts and update checks..."
        beforepaste uninstall 2>/dev/null || true
    fi
}

remove_binary() {
    echo -e "  ${YELLOW}[~]${NC} Removing CLI binaries..."
    sudo rm -f /usr/local/bin/beforepaste 2>/dev/null || true
    rm -f "$HOME/.local/bin/beforepaste" 2>/dev/null || true
}

remove_config() {
    echo -e "  ${YELLOW}[~]${NC} Removing local configuration..."
    rm -rf "$HOME/.config/beforewire/beforepaste" 2>/dev/null || true
    rm -rf "$HOME/Library/Application Support/beforewire/beforepaste" 2>/dev/null || true
    if [ -n "${APPDATA:-}" ]; then
        rm -rf "$APPDATA/beforewire/beforepaste" 2>/dev/null || true
    fi
}

remove_service_files() {
    echo -e "  ${YELLOW}[~]${NC} Removing update-check service files..."
    rm -f "$HOME/.config/systemd/user/beforepaste-update-check.service" 2>/dev/null || true
    rm -f "$HOME/.config/systemd/user/beforepaste-update-check.timer" 2>/dev/null || true
    rm -f "$HOME/Library/LaunchAgents/com.beforewire.beforepaste-update-check.plist" 2>/dev/null || true
}

main() {
    run_cli_uninstall
    remove_binary
    remove_service_files
    remove_config
    echo ""
    echo -e "${GREEN}[OK]${NC} BeforePaste CLI files and local config have been removed."
    echo -e "  ${YELLOW}If you installed the desktop app, remove it from Applications or your package manager as usual.${NC}"
    echo ""
}

main "$@"
