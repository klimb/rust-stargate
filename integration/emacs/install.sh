#!/bin/bash
# Stargate Shell Emacs Mode Installer
# This script installs stargate-shell.el to your Emacs configuration

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default installation directory
EMACS_DIR="${HOME}/.emacs.d/lisp"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STARGATE_EL="${SCRIPT_DIR}/stargate-shell.el"
STARGATE_SCRIPT_EL="${SCRIPT_DIR}/stargate-script-mode.el"
EXAMPLE_CONFIG="${SCRIPT_DIR}/example.emacs"

# Find stargate-shell binary
STARGATE_BIN=""
if [ -f "${SCRIPT_DIR}/../../target/debug/stargate-shell" ]; then
    STARGATE_BIN="${SCRIPT_DIR}/../../target/debug/stargate-shell"
elif [ -f "${SCRIPT_DIR}/../../target/release/stargate-shell" ]; then
    STARGATE_BIN="${SCRIPT_DIR}/../../target/release/stargate-shell"
elif command -v stargate-shell &> /dev/null; then
    STARGATE_BIN="$(command -v stargate-shell)"
fi

echo -e "${GREEN}Stargate Shell Emacs Mode Installer${NC}"
echo "===================================="
echo

# Check if stargate-shell.el exists
if [ ! -f "$STARGATE_EL" ]; then
    echo -e "${RED}Error: stargate-shell.el not found at ${STARGATE_EL}${NC}"
    exit 1
fi

if [ ! -f "$STARGATE_SCRIPT_EL" ]; then
    echo -e "${RED}Error: stargate-script-mode.el not found at ${STARGATE_SCRIPT_EL}${NC}"
    exit 1
fi

# Create Emacs lisp directory if it doesn't exist
if [ ! -d "$EMACS_DIR" ]; then
    echo -e "${YELLOW}Creating directory: ${EMACS_DIR}${NC}"
    mkdir -p "$EMACS_DIR"
fi

# Copy stargate-shell.el
echo "Installing stargate-shell.el to ${EMACS_DIR}/"
cp "$STARGATE_EL" "$EMACS_DIR/"
echo -e "${GREEN}✓ Installed stargate-shell.el${NC}"

# Copy stargate-script-mode.el
echo "Installing stargate-script-mode.el to ${EMACS_DIR}/"
cp "$STARGATE_SCRIPT_EL" "$EMACS_DIR/"
echo -e "${GREEN}✓ Installed stargate-script-mode.el${NC}"

# Detect init file
INIT_FILE=""
if [ -f "${HOME}/.emacs" ]; then
    INIT_FILE="${HOME}/.emacs"
elif [ -f "${HOME}/.emacs.d/init.el" ]; then
    INIT_FILE="${HOME}/.emacs.d/init.el"
fi

# Check if already configured
HAS_SHELL_MODE=false
HAS_SCRIPT_MODE=false
if [ -n "$INIT_FILE" ]; then
    if grep -q "require 'stargate-shell" "$INIT_FILE" 2>/dev/null; then
        HAS_SHELL_MODE=true
    fi
    if grep -q "require 'stargate-script-mode" "$INIT_FILE" 2>/dev/null; then
        HAS_SCRIPT_MODE=true
    fi
fi

# If script mode is missing but shell mode exists, add it
if [ "$HAS_SHELL_MODE" = true ] && [ "$HAS_SCRIPT_MODE" = false ] && [ -n "$INIT_FILE" ]; then
    echo -e "${YELLOW}Adding stargate-script-mode to existing configuration...${NC}"
    # Find the line with stargate-shell and add script-mode after it
    sed -i "/require 'stargate-shell)/a (require 'stargate-script-mode)" "$INIT_FILE"
    echo -e "${GREEN}✓ Added stargate-script-mode to ${INIT_FILE}${NC}"
    HAS_SCRIPT_MODE=true
fi

if [ "$HAS_SHELL_MODE" = true ] && [ "$HAS_SCRIPT_MODE" = true ]; then
    echo -e "${GREEN}✓ Emacs configuration already contains both stargate modes${NC}"
elif [ "$HAS_SHELL_MODE" = false ]; then
    echo
    echo "Add this to your Emacs configuration:"
    if [ -n "$INIT_FILE" ]; then
        echo -e "${YELLOW}File: ${INIT_FILE}${NC}"
    else
        echo -e "${YELLOW}File: ~/.emacs.d/init.el (or ~/.emacs)${NC}"
    fi
    echo
    echo "----------------------------------------"
    echo ";; Stargate Shell mode"
    echo "(add-to-list 'load-path \"${EMACS_DIR}\")"
    echo "(require 'stargate-shell)"
    echo "(require 'stargate-script-mode)"
    if [ -n "$STARGATE_BIN" ]; then
        echo "(setq stargate-shell-program \"${STARGATE_BIN}\")"
    else
        echo "(setq stargate-shell-program \"stargate-shell\")"
    fi
    echo "(global-set-key (kbd \"C-c s\") 'stargate-shell)"
    echo "----------------------------------------"
    echo
    
    # Offer to add configuration automatically
    if [ -n "$INIT_FILE" ]; then
        read -p "Add this configuration to ${INIT_FILE} automatically? [y/N] " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            echo "" >> "$INIT_FILE"
            echo ";; Stargate Shell mode (added by install.sh)" >> "$INIT_FILE"
            echo "(add-to-list 'load-path \"${EMACS_DIR}\")" >> "$INIT_FILE"
            echo "(require 'stargate-shell)" >> "$INIT_FILE"
            echo "(require 'stargate-script-mode)" >> "$INIT_FILE"
            if [ -n "$STARGATE_BIN" ]; then
                echo "(setq stargate-shell-program \"${STARGATE_BIN}\")" >> "$INIT_FILE"
            else
                echo "(setq stargate-shell-program \"stargate-shell\")" >> "$INIT_FILE"
            fi
            echo "(global-set-key (kbd \"C-c s\") 'stargate-shell)" >> "$INIT_FILE"
            echo -e "${GREEN}✓ Configuration added to ${INIT_FILE}${NC}"
        fi
    fi
fi

echo
echo -e "${GREEN}Installation complete!${NC}"
echo
echo "Usage:"
echo "  1. Restart Emacs (or run: M-x eval-buffer in your init file)"
echo "  2. Press C-c s (or run: M-x stargate-shell)"
echo
echo "For more information, see: ${SCRIPT_DIR}/README.md"

if [ -z "$STARGATE_BIN" ]; then
    echo
    echo -e "${YELLOW}Warning: stargate-shell binary not found${NC}"
    echo "Please build it with: cargo build --release --bin stargate-shell"
    echo "Or install it to your PATH"
fi
