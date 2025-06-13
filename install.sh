#!/bin/sh
# ccost installer
#
# Usage:
#   curl -sSf https://raw.githubusercontent.com/carlosarraes/ccost/main/install.sh | sh

set -e

REPO="carlosarraes/ccost"
BINARY_NAME="ccost"
BIN_DIR="${BIN_DIR:-$HOME/.local/bin}"
GITHUB_LATEST="https://api.github.com/repos/${REPO}/releases/latest"

get_arch() {
  # detect architecture
  ARCH=$(uname -m)
  case $ARCH in
  x86_64) ARCH="x86_64" ;;
  aarch64) ARCH="aarch64" ;;
  arm64) ARCH="aarch64" ;;
  *)
    echo "Unsupported architecture: $ARCH"
    exit 1
    ;;
  esac
}

get_os() {
  # detect os
  OS=$(uname -s)
  case $OS in
  Linux) OS="linux" ;;
  Darwin) OS="darwin" ;;
  *)
    echo "Unsupported OS: $OS"
    exit 1
    ;;
  esac
}

download_binary() {
  # get latest release info
  echo "Fetching latest release..."
  VERSION=$(curl -s $GITHUB_LATEST | grep -o '"tag_name": "[^"]*' | cut -d'"' -f4)
  if [ -z "$VERSION" ]; then
    echo "Failed to fetch latest version"
    exit 1
  fi

  echo "Latest version: $VERSION"

  # create temporary directory
  TMP_DIR=$(mktemp -d)
  # Ensure cleanup happens even if script fails or exits early
  trap 'rm -rf "$TMP_DIR"' EXIT

  echo "Downloading ${BINARY_NAME} ${VERSION}..."

  # Download the binary directly (no tar.gz)
  # Try generic binary first, then platform-specific
  DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${BINARY_NAME}"
  PLATFORM_BINARY="${BINARY_NAME}-${OS}-${ARCH}"
  PLATFORM_URL="https://github.com/${REPO}/releases/download/${VERSION}/${PLATFORM_BINARY}"

  # download the binary directly
  echo "Downloading from: $DOWNLOAD_URL"
  if ! curl -fsSL "$DOWNLOAD_URL" -o "${TMP_DIR}/${BINARY_NAME}" 2>/dev/null; then
    echo "Generic binary not found, trying platform-specific binary..."
    echo "Downloading from: $PLATFORM_URL"
    curl -fsSL "$PLATFORM_URL" -o "${TMP_DIR}/${BINARY_NAME}" || {
      echo "Download failed. Check URL/permissions/network."
      exit 1
    }
  fi

  # make it executable
  chmod +x "${TMP_DIR}/${BINARY_NAME}"

  # Check if BIN_DIR exists and create if needed
  CREATED_DIR_MSG=""
  if [ ! -d "$BIN_DIR" ]; then
    echo "Installation directory '$BIN_DIR' not found."
    echo "Creating directory: $BIN_DIR"
    mkdir -p "$BIN_DIR"
    CREATED_DIR_MSG="Note: Created directory '$BIN_DIR'. You might need to add it to your system's PATH."
  fi

  # install binary (no sudo needed for $HOME/.local/bin)
  echo "Installing to $BIN_DIR..."
  install -m 755 "${TMP_DIR}/${BINARY_NAME}" "$BIN_DIR"

  # cleanup happens via trap

  echo "${BINARY_NAME} ${VERSION} installed successfully to $BIN_DIR"

  # Print the warning message if the directory was created
  if [ -n "$CREATED_DIR_MSG" ]; then
    echo ""
    echo "$CREATED_DIR_MSG"
  fi
}

# Run the installer
get_arch
get_os
download_binary

echo ""
echo "Installation complete! Run '${BINARY_NAME} --help' to get started."