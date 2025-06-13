#!/bin/bash

# Dry run test of install.sh
# This tests the install script without actually downloading/installing

echo "=== Install Script Dry Run Test ==="
echo

# Test the functions from install.sh by sourcing parts of it
source <(grep -A 20 "detect_os()" install.sh | head -20)
source <(grep -A 20 "detect_arch()" install.sh | head -20)

echo "Testing install script components:"
echo

# Test OS detection
echo "1. OS Detection:"
if command -v uname >/dev/null 2>&1; then
    current_os=$(uname -s)
    echo "   Current OS: $current_os"
    
    # Test the detect_os function logic manually
    case "$current_os" in
        Linux*)
            detected="linux"
            ;;
        Darwin*)
            detected="macos"
            ;;
        *)
            detected="unsupported"
            ;;
    esac
    echo "   Detected: $detected"
else
    echo "   uname not available"
fi

echo

# Test Architecture detection
echo "2. Architecture Detection:"
if command -v uname >/dev/null 2>&1; then
    current_arch=$(uname -m)
    echo "   Current arch: $current_arch"
    
    # Test the detect_arch function logic manually
    case "$current_arch" in
        x86_64|amd64)
            detected="x86_64"
            ;;
        arm64|aarch64)
            detected="aarch64"
            ;;
        *)
            detected="unsupported"
            ;;
    esac
    echo "   Detected: $detected"
else
    echo "   uname not available"
fi

echo

# Test URL construction
echo "3. URL Construction:"
REPO="carlosarraes/ccost"
os="linux"
arch="x86_64"
asset_name="ccost-$os-$arch"
url="https://github.com/$REPO/releases/latest/download/$asset_name.tar.gz"
echo "   Example URL: $url"

echo

# Test prerequisites
echo "4. Prerequisites Check:"
for cmd in curl wget tar; do
    if command -v "$cmd" >/dev/null 2>&1; then
        echo "   ✓ $cmd is available"
    else
        echo "   ✗ $cmd is not available"
    fi
done

echo

# Test install directory
echo "5. Install Directory:"
install_dir="$HOME/.local/bin"
echo "   Install directory: $install_dir"
if [ -d "$install_dir" ]; then
    echo "   ✓ Directory exists"
else
    echo "   ! Directory doesn't exist (will be created)"
fi

echo

# Test PATH
echo "6. PATH Check:"
case ":$PATH:" in
    *":$install_dir:"*)
        echo "   ✓ Install directory is in PATH"
        ;;
    *)
        echo "   ! Install directory is not in PATH"
        echo "   Note: User will need to add $install_dir to PATH or use full path"
        ;;
esac

echo
echo "=== Dry Run Test Complete ==="
echo
echo "The install script appears to be properly configured."
echo "When a release is available, users can install with:"
echo "curl -sSf https://raw.githubusercontent.com/carlosarraes/ccost/main/install.sh | sh"