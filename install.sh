#!/usr/bin/env bash
# ==============================================================================
# τ Tau (tau) 1-Line Installer
# High-Performance Autonomous Coding Agent (2π Evolution of Pi)
# ==============================================================================

set -euo pipefail

# ANSI Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'

# Disable colors if stdout is not a TTY
if [ ! -t 1 ]; then
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    MAGENTA=''
    CYAN=''
    BOLD=''
    DIM=''
    RESET=''
fi

TAU_HOME="${HOME}/.tau"
PI_HOME="${HOME}/.pi"

echo -e "${CYAN}"
echo -e "  ╔══════════════════════════════════════════════════════════════════════════════╗"
echo -e "  ║                                                                              ║"
echo -e "  ║     ${MAGENTA}τ  T A U${CYAN}   (2π Evolution of Pi)                                           ║"
echo -e "  ║     ${BOLD}1-Line Autonomous Coding Agent Installer${RESET}${CYAN}                                 ║"
echo -e "  ║                                                                              ║"
echo -e "  ╚══════════════════════════════════════════════════════════════════════════════╝"
echo -e "${RESET}"

# 1. Detect Operating System and Architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

echo -e "${BOLD}▶ [1/4] Detecting Platform...${RESET}"
case "${OS}" in
    Darwin)
        OS_NAME="macOS"
        ;;
    Linux)
        OS_NAME="Linux"
        ;;
    *)
        echo -e "${RED}✗ Error: Unsupported operating system: ${OS}${RESET}"
        echo -e "pi-rust currently supports macOS (Darwin) and Linux."
        exit 1
        ;;
esac

case "${ARCH}" in
    x86_64|amd64)
        ARCH_NAME="x86_64"
        ;;
    arm64|aarch64)
        ARCH_NAME="aarch64"
        ;;
    *)
        echo -e "${RED}✗ Error: Unsupported architecture: ${ARCH}${RESET}"
        exit 1
        ;;
esac

echo -e "  ${GREEN}✓${RESET} Platform: ${BOLD}${OS_NAME} (${ARCH_NAME})${RESET}"

# 2. Setup Destination Paths
echo -e "\n${BOLD}▶ [2/4] Setting Up Installation Directories...${RESET}"
PI_HOME="${HOME}/.pi"
BIN_DIR="${PI_HOME}/bin"
mkdir -p "${BIN_DIR}"
mkdir -p "${PI_HOME}/agent"

TARGET_BIN="${BIN_DIR}/pi-rs"
echo -e "  ${GREEN}✓${RESET} Target Binary Directory: ${BOLD}${BIN_DIR}${RESET}"

# 3. Build or Install Binary
echo -e "\n${BOLD}▶ [3/4] Installing pi-rs Binary...${RESET}"

# Check if we are inside the pi-rust repository
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -f "${SCRIPT_DIR}/Cargo.toml" ] && grep -q "pi-cli" "${SCRIPT_DIR}/Cargo.toml" 2>/dev/null; then
    echo -e "  ${BLUE}●${RESET} Found local pi-rust repository at ${SCRIPT_DIR}"
    echo -e "  ${BLUE}●${RESET} Building release binary with cargo..."
    cargo build --release --package pi-cli --bin pi-rs --manifest-path "${SCRIPT_DIR}/Cargo.toml"
    cp -f "${SCRIPT_DIR}/target/release/pi-rs" "${TARGET_BIN}"
    chmod +x "${TARGET_BIN}"
    echo -e "  ${GREEN}✓${RESET} Built and installed local binary to ${BOLD}${TARGET_BIN}${RESET}"
elif command -v cargo >/dev/null 2>&1; then
    echo -e "  ${BLUE}●${RESET} Compiling and installing pi-rs using cargo..."
# 3. Build / Install Binary
echo -e "\n${BOLD}▶ [3/4] Installing Tau (${TAU_HOME}/bin)...${RESET}"
mkdir -p "${TAU_HOME}/bin"
mkdir -p "${HOME}/.pi/bin"
TARGET_BIN="${TAU_HOME}/bin/tau"
TARGET_PIRS="${TAU_HOME}/bin/pi-rs"

if command -v cargo >/dev/null 2>&1; then
    echo -e "  ${DIM}Building release binary from source via cargo...${RESET}"
    if [ -f "./Cargo.toml" ]; then
        cargo build --release --bin tau --bin pi-rs
        cp -f "./target/release/tau" "${TARGET_BIN}"
        cp -f "./target/release/pi-rs" "${TARGET_PIRS}"
        cp -f "./target/release/tau" "${HOME}/.pi/bin/tau" 2>/dev/null || true
        cp -f "./target/release/pi-rs" "${HOME}/.pi/bin/pi-rs" 2>/dev/null || true
    else
        cargo install --git https://github.com/earendil-works/pi-rust.git pi-cli --bin tau --bin pi-rs --root "${TAU_HOME}" --force
    fi
    chmod +x "${TARGET_BIN}" "${TARGET_PIRS}"
    echo -e "  ${GREEN}✓${RESET} Installed binary to ${BOLD}${TARGET_BIN}${RESET}"
    echo -e "  ${GREEN}✓${RESET} Created compatibility alias ${BOLD}${TARGET_PIRS}${RESET}"
else
    echo -e "${RED}✗ Cargo is required to build Tau from source.${RESET}"
    echo -e "Please install Rust & Cargo via: ${BOLD}curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh${RESET}"
    exit 1
fi

# 4. Configure PATH in User Shell Profiles
echo -e "\n${BOLD}▶ [4/4] Configuring Environment PATH...${RESET}"
CONFIGURED_SHELLS=0

add_to_path() {
    local rc_file="$1"
    local export_line="export PATH=\"\$HOME/.tau/bin:\$HOME/.pi/bin:\$PATH\""

    if [ -f "${rc_file}" ]; then
        if grep -q "\.tau/bin" "${rc_file}"; then
            echo -e "  ${DIM}• ${rc_file} already contains .tau/bin in PATH${RESET}"
        else
            echo "" >> "${rc_file}"
            echo "# Tau (τ) autonomous agent binary path" >> "${rc_file}"
            echo "${export_line}" >> "${rc_file}"
            echo -e "  ${GREEN}✓${RESET} Added ~/.tau/bin to ${BOLD}${rc_file}${RESET}"
            CONFIGURED_SHELLS=$((CONFIGURED_SHELLS + 1))
        fi
    fi
}

# Update standard shell profile files
add_to_path "${HOME}/.zshrc"
add_to_path "${HOME}/.bashrc"
add_to_path "${HOME}/.bash_profile"
add_to_path "${HOME}/.profile"

# Fish shell support
if [ -d "${HOME}/.config/fish" ]; then
    FISH_CONFIG="${HOME}/.config/fish/config.fish"
    if [ -f "${FISH_CONFIG}" ]; then
        if ! grep -q "\.tau/bin" "${FISH_CONFIG}"; then
            echo "" >> "${FISH_CONFIG}"
            echo "# Tau (τ) autonomous agent binary path" >> "${FISH_CONFIG}"
            echo 'set -gx PATH $HOME/.tau/bin $HOME/.pi/bin $PATH' >> "${FISH_CONFIG}"
            echo -e "  ${GREEN}✓${RESET} Added ~/.tau/bin to ${BOLD}${FISH_CONFIG}${RESET}"
            CONFIGURED_SHELLS=$((CONFIGURED_SHELLS + 1))
        fi
    fi
fi

# 5. Verification
echo -e "\n${BOLD}▶ Verifying Installation...${RESET}"
if [ -x "${TARGET_BIN}" ]; then
    echo -e "  ${GREEN}✓${RESET} Binary executable verified: ${BOLD}${TARGET_BIN}${RESET}"
    VERSION_OUTPUT="$("${TARGET_BIN}" --version || true)"
    echo -e "  ${DIM}${VERSION_OUTPUT}${RESET}"
else
    echo -e "${RED}✗ Error: Installed binary at ${TARGET_BIN} is not executable.${RESET}"
    exit 1
fi

echo -e "\n${GREEN}"
echo -e "  ╔══════════════════════════════════════════════════════════════════════════════╗"
echo -e "  ║  ✓ Tau (τ) Installation Complete!                                            ║"
echo -e "  ╚══════════════════════════════════════════════════════════════════════════════╝"
echo -e "${RESET}"

echo -e "  ${BOLD}Next Steps:${RESET}"
echo -e "    1. Reload your current shell environment:"
if [ -f "${HOME}/.zshrc" ]; then
    echo -e "       ${CYAN}source ~/.zshrc${RESET}"
elif [ -f "${HOME}/.bashrc" ]; then
    echo -e "       ${CYAN}source ~/.bashrc${RESET}"
fi
echo -e "    2. Run the first-time setup onboarding wizard:"
echo -e "       ${CYAN}tau --init${RESET}"
echo -e "    3. Or launch directly into the autonomous coding TUI:"
echo -e "       ${CYAN}tau${RESET}"
echo -e ""

