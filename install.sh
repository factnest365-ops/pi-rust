#!/usr/bin/env bash
# ==============================================================================
# τ Tau Installer — Zero-Dependency Rust Native Agent
# ==============================================================================

set -euo pipefail

BOLD='\033[1m'
DIM='\033[2m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
RED='\033[0;31m'
RESET='\033[0m'

if [ ! -t 1 ]; then
    BOLD=''
    DIM=''
    GREEN=''
    CYAN=''
    RED=''
    RESET=''
fi

echo -e "${BOLD}τ Tau${RESET} ${DIM}— Installing High-Performance Autonomous Coding Agent...${RESET}\n"

# 1. Platform Detection
OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}" in
    Darwin) OS_NAME="macOS" ;;
    Linux)  OS_NAME="Linux" ;;
    *)
        echo -e "${RED}Error: Unsupported operating system: ${OS}${RESET}"
        exit 1
        ;;
esac

case "${ARCH}" in
    x86_64|amd64) ARCH_NAME="x86_64" ;;
    arm64|aarch64) ARCH_NAME="aarch64" ;;
    *)
        echo -e "${RED}Error: Unsupported architecture: ${ARCH}${RESET}"
        exit 1
        ;;
esac

echo -e "  ${GREEN}✓${RESET} Platform: ${OS_NAME} (${ARCH_NAME})"

# 2. Binary Destination
TAU_HOME="${HOME}/.tau"
PI_HOME="${HOME}/.pi"
BIN_DIR="${TAU_HOME}/bin"
mkdir -p "${BIN_DIR}"
mkdir -p "${PI_HOME}/bin"

TARGET_TAU="${BIN_DIR}/tau"
TARGET_PIRS="${BIN_DIR}/pi-rs"

# 3. Build & Install
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -f "${SCRIPT_DIR}/Cargo.toml" ]; then
    echo -e "  ${DIM}Building release binaries from local source...${RESET}"
    cargo build --release --manifest-path "${SCRIPT_DIR}/Cargo.toml" --bin tau --bin pi-rs
    cp -f "${SCRIPT_DIR}/target/release/tau" "${TARGET_TAU}"
    cp -f "${SCRIPT_DIR}/target/release/pi-rs" "${TARGET_PIRS}"
    cp -f "${SCRIPT_DIR}/target/release/tau" "${PI_HOME}/bin/tau" 2>/dev/null || true
    cp -f "${SCRIPT_DIR}/target/release/pi-rs" "${PI_HOME}/bin/pi-rs" 2>/dev/null || true
    if [ -d "${HOME}/.cargo/bin" ]; then
        cp -f "${SCRIPT_DIR}/target/release/tau" "${HOME}/.cargo/bin/tau" 2>/dev/null || true
        cp -f "${SCRIPT_DIR}/target/release/pi-rs" "${HOME}/.cargo/bin/pi-rs" 2>/dev/null || true
    fi
elif command -v cargo >/dev/null 2>&1; then
    echo -e "  ${DIM}Compiling tau via cargo...${RESET}"
    cargo install --git https://github.com/earendil-works/pi-rust.git pi-cli --bin tau --bin pi-rs --root "${TAU_HOME}" --force
    if [ -d "${HOME}/.cargo/bin" ]; then
        cp -f "${TARGET_TAU}" "${HOME}/.cargo/bin/tau" 2>/dev/null || true
        cp -f "${TARGET_PIRS}" "${HOME}/.cargo/bin/pi-rs" 2>/dev/null || true
    fi
else
    echo -e "${RED}Error: Cargo is required to build Tau from source.${RESET}"
    echo -e "Install Rust via: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

chmod +x "${TARGET_TAU}" "${TARGET_PIRS}"
echo -e "  ${GREEN}✓${RESET} Installed: ${BOLD}${TARGET_TAU}${RESET}"
echo -e "  ${GREEN}✓${RESET} Alias:     ${BOLD}${TARGET_PIRS}${RESET}"

# 4. PATH Configuration
add_path_if_missing() {
    local rc="$1"
    local line='export PATH="$HOME/.tau/bin:$HOME/.pi/bin:$PATH"'
    if [ -f "${rc}" ]; then
        if ! grep -q "\.tau/bin" "${rc}" 2>/dev/null; then
            echo "" >> "${rc}"
            echo "# Tau autonomous agent" >> "${rc}"
            echo "${line}" >> "${rc}"
            echo -e "  ${GREEN}✓${RESET} Updated PATH in ${DIM}${rc}${RESET}"
        fi
    fi
}

add_path_if_missing "${HOME}/.zshrc"
add_path_if_missing "${HOME}/.bashrc"
add_path_if_missing "${HOME}/.bash_profile"
add_path_if_missing "${HOME}/.profile"

if [ -d "${HOME}/.config/fish" ] && [ -f "${HOME}/.config/fish/config.fish" ]; then
    FISH_RC="${HOME}/.config/fish/config.fish"
    if ! grep -q "\.tau/bin" "${FISH_RC}" 2>/dev/null; then
        echo "" >> "${FISH_RC}"
        echo 'set -gx PATH $HOME/.tau/bin $HOME/.pi/bin $PATH' >> "${FISH_RC}"
        echo -e "  ${GREEN}✓${RESET} Updated PATH in ${DIM}${FISH_RC}${RESET}"
    fi
fi

# 5. Summary
echo -e "\n${GREEN}✓ Installation complete.${RESET}"
if [ -f "${HOME}/.zshrc" ]; then
    echo -e "  ${DIM}To use in this shell session, run:${RESET} ${BOLD}source ~/.zshrc${RESET}"
fi
echo -e "  Run ${CYAN}tau${RESET} to start, or ${CYAN}tau --help${RESET} for commands.\n"
