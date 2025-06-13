#!/bin/bash

# ccost installer script
# This script installs ccost from GitHub releases
# Usage: curl -sSf https://raw.githubusercontent.com/carlosarraes/ccost/main/install.sh | sh

set -e

# Configuration
REPO="carlosarraes/ccost"
BINARY_NAME="ccost"
INSTALL_DIR="$HOME/.local/bin"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

# Error handling
cleanup() {
    if [ -n "$temp_dir" ] && [ -d "$temp_dir" ]; then
        log_info "Cleaning up temporary files..."
        rm -rf "$temp_dir"
    fi
}

error_exit() {
    log_error "$1"
    cleanup
    exit 1
}

# Trap errors and cleanup
trap cleanup EXIT
trap 'error_exit "Installation interrupted"' INT TERM

# Detect OS
detect_os() {
    local os
    os=$(uname -s)
    case "$os" in
        Linux*)
            echo "linux"
            ;;
        Darwin*)
            echo "macos"
            ;;
        CYGWIN*|MINGW*)
            error_exit "Windows is not supported yet. Please check GitHub releases for Windows binaries."
            ;;
        *)
            error_exit "Unsupported operating system: $os"
            ;;
    esac
}

# Detect architecture
detect_arch() {
    local arch
    arch=$(uname -m)
    case "$arch" in
        x86_64|amd64)
            echo "x86_64"
            ;;
        arm64|aarch64)
            echo "aarch64"
            ;;
        *)
            error_exit "Unsupported architecture: $arch"
            ;;
    esac
}

# Check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    if ! command_exists curl && ! command_exists wget; then
        error_exit "Neither curl nor wget is available. Please install one of them to continue."
    fi
    
    if ! command_exists tar; then
        error_exit "tar is required but not available. Please install tar to continue."
    fi
    
    log_success "Prerequisites check passed"
}

# Validate OS/arch combination
validate_platform() {
    local os="$1"
    local arch="$2"
    
    # Linux currently only supports x86_64
    if [ "$os" = "linux" ] && [ "$arch" = "aarch64" ]; then
        error_exit "Linux ARM64 is not supported yet. Only Linux x86_64 is available."
    fi
    
    log_info "Platform validated: $os-$arch"
}

# Download file with fallback
download_file() {
    local url="$1"
    local output="$2"
    
    log_info "Downloading from: $url"
    
    if command_exists curl; then
        if ! curl -sSfL "$url" -o "$output"; then
            return 1
        fi
    elif command_exists wget; then
        if ! wget -q "$url" -O "$output"; then
            return 1
        fi
    else
        error_exit "No download tool available"
    fi
    
    return 0
}

# Get latest release info
get_latest_release() {
    local api_url="https://api.github.com/repos/$REPO/releases/latest"
    local release_info
    
    log_info "Fetching latest release information..."
    
    if command_exists curl; then
        release_info=$(curl -sSf "$api_url" 2>/dev/null) || {
            log_warn "Failed to fetch release info from API, using 'latest' tag"
            echo "latest"
            return 0
        }
    elif command_exists wget; then
        release_info=$(wget -qO- "$api_url" 2>/dev/null) || {
            log_warn "Failed to fetch release info from API, using 'latest' tag"
            echo "latest"
            return 0
        }
    else
        log_warn "No download tool available for API call, using 'latest' tag"
        echo "latest"
        return 0
    fi
    
    # Extract tag name from JSON (simple grep approach)
    local tag_name
    tag_name=$(echo "$release_info" | grep '"tag_name"' | head -n 1 | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')
    
    if [ -n "$tag_name" ]; then
        echo "$tag_name"
    else
        log_warn "Could not parse release tag, using 'latest'"
        echo "latest"
    fi
}

# Main installation function
install_ccost() {
    log_info "Starting ccost installation..."
    
    # Detect platform
    local os arch
    os=$(detect_os)
    arch=$(detect_arch)
    validate_platform "$os" "$arch"
    
    # Check prerequisites
    check_prerequisites
    
    # Get latest release
    local version
    version=$(get_latest_release)
    log_info "Installing ccost version: $version"
    
    # Construct download URL
    local asset_name="ccost-$os-$arch"
    local download_url
    if [ "$version" = "latest" ]; then
        download_url="https://github.com/$REPO/releases/latest/download/$asset_name.tar.gz"
    else
        download_url="https://github.com/$REPO/releases/download/$version/$asset_name.tar.gz"
    fi
    
    # Create temporary directory
    temp_dir=$(mktemp -d)
    log_info "Using temporary directory: $temp_dir"
    
    # Download archive
    local archive_path="$temp_dir/$asset_name.tar.gz"
    if ! download_file "$download_url" "$archive_path"; then
        error_exit "Failed to download ccost from $download_url. Please check your internet connection and try again."
    fi
    
    log_success "Downloaded ccost archive"
    
    # Extract archive
    log_info "Extracting archive..."
    if ! tar -xzf "$archive_path" -C "$temp_dir"; then
        error_exit "Failed to extract archive. The download may be corrupted."
    fi
    
    # Verify binary exists
    local binary_path="$temp_dir/$BINARY_NAME"
    if [ ! -f "$binary_path" ]; then
        error_exit "Binary not found in archive. Expected: $BINARY_NAME"
    fi
    
    # Make binary executable
    chmod +x "$binary_path"
    
    # Test binary
    log_info "Testing binary..."
    if ! "$binary_path" --version >/dev/null 2>&1; then
        log_warn "Binary version check failed, but continuing installation..."
    fi
    
    # Create installation directory
    log_info "Creating installation directory: $INSTALL_DIR"
    mkdir -p "$INSTALL_DIR"
    
    # Install binary
    local install_path="$INSTALL_DIR/$BINARY_NAME"
    log_info "Installing binary to: $install_path"
    
    if ! cp "$binary_path" "$install_path"; then
        error_exit "Failed to copy binary to $install_path. Check permissions."
    fi
    
    log_success "ccost installed successfully!"
    
    # Check PATH
    case ":$PATH:" in
        *":$INSTALL_DIR:"*)
            log_success "Installation directory is already in PATH"
            ;;
        *)
            log_warn "Installation directory is not in PATH"
            echo
            echo "To use ccost, add the following to your shell profile:"
            echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
            echo
            echo "Or run ccost directly:"
            echo "  $install_path --help"
            ;;
    esac
    
    echo
    log_success "Installation complete!"
    echo
    echo "Try running: ccost --help"
    echo "Or if not in PATH: $install_path --help"
    echo
    echo "For more information, visit: https://github.com/$REPO"
}

# Run installation
install_ccost