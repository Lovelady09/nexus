#!/bin/sh
# Generate macOS application assets from SVG source
# Requires: ImageMagick (magick/convert)
# Optional: libicns (png2icns) for ICNS generation

set -e

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SVG_SOURCE="${SCRIPT_DIR}/../linux/nexus.svg"
MACOS_DIR="${SCRIPT_DIR}"
ICONSET_DIR="${MACOS_DIR}/nexus.iconset"

# Check if SVG source exists
if [ ! -f "$SVG_SOURCE" ]; then
    echo "Error: SVG source not found at $SVG_SOURCE" >&2
    exit 1
fi

# Check for required tools
if ! command -v magick >/dev/null 2>&1 && ! command -v convert >/dev/null 2>&1; then
    echo "Error: ImageMagick not found (need 'magick' or 'convert' command)" >&2
    echo "Install with: brew install imagemagick (macOS)" >&2
    echo "            or pacman -S imagemagick (Arch)" >&2
    echo "            or apt install imagemagick (Debian/Ubuntu)" >&2
    echo "            or dnf install imagemagick (Fedora)" >&2
    exit 1
fi

# Determine which ImageMagick command to use
if command -v magick >/dev/null 2>&1; then
    CONVERT_CMD="magick"
else
    CONVERT_CMD="convert"
fi

# Check for iconutil (macOS) or png2icns (Linux)
USE_ICONUTIL=""
USE_PNG2ICNS=""
if command -v iconutil >/dev/null 2>&1; then
    USE_ICONUTIL="1"
elif command -v png2icns >/dev/null 2>&1; then
    USE_PNG2ICNS="1"
else
    echo "Warning: Neither iconutil nor png2icns found - skipping ICNS generation" >&2
    echo "On macOS: iconutil is built-in" >&2
    echo "On Linux: Install libicns (png2icns)" >&2
    echo "  - Arch: pacman -S libicns" >&2
    echo "  - Debian/Ubuntu: apt install icnsutils" >&2
    echo "  - Fedora: dnf install libicns-utils" >&2
fi

echo "Generating macOS assets from $SVG_SOURCE"
echo ""

# Generate macOS PNG (1024x1024) with transparency
echo "Generating PNG (1024x1024)..."
"$CONVERT_CMD" -background none "$SVG_SOURCE" -resize 1024x1024 "${MACOS_DIR}/nexus.png"
echo "✓ nexus.png"

# Generate ICNS with all required sizes
if [ -n "$USE_ICONUTIL" ]; then
    # macOS: Use iconutil with .iconset folder
    echo "Generating macOS ICNS (using iconutil)..."
    
    # Create iconset directory
    rm -rf "$ICONSET_DIR"
    mkdir -p "$ICONSET_DIR"
    
    # Generate all required sizes
    # Standard (1x) icons
    "$CONVERT_CMD" -background none "$SVG_SOURCE" -resize 16x16     "${ICONSET_DIR}/icon_16x16.png"
    "$CONVERT_CMD" -background none "$SVG_SOURCE" -resize 32x32     "${ICONSET_DIR}/icon_32x32.png"
    "$CONVERT_CMD" -background none "$SVG_SOURCE" -resize 128x128   "${ICONSET_DIR}/icon_128x128.png"
    "$CONVERT_CMD" -background none "$SVG_SOURCE" -resize 256x256   "${ICONSET_DIR}/icon_256x256.png"
    "$CONVERT_CMD" -background none "$SVG_SOURCE" -resize 512x512   "${ICONSET_DIR}/icon_512x512.png"
    
    # Retina (2x) icons - these are named @2x and are double the base size
    "$CONVERT_CMD" -background none "$SVG_SOURCE" -resize 32x32     "${ICONSET_DIR}/icon_16x16@2x.png"
    "$CONVERT_CMD" -background none "$SVG_SOURCE" -resize 64x64     "${ICONSET_DIR}/icon_32x32@2x.png"
    "$CONVERT_CMD" -background none "$SVG_SOURCE" -resize 256x256   "${ICONSET_DIR}/icon_128x128@2x.png"
    "$CONVERT_CMD" -background none "$SVG_SOURCE" -resize 512x512   "${ICONSET_DIR}/icon_256x256@2x.png"
    "$CONVERT_CMD" -background none "$SVG_SOURCE" -resize 1024x1024 "${ICONSET_DIR}/icon_512x512@2x.png"
    
    # Convert iconset to icns
    iconutil -c icns -o "${MACOS_DIR}/nexus.icns" "$ICONSET_DIR"
    
    # Clean up iconset directory
    rm -rf "$ICONSET_DIR"
    
    echo "✓ nexus.icns (with all sizes: 16, 32, 128, 256, 512, 1024)"

elif [ -n "$USE_PNG2ICNS" ]; then
    # Linux: Use png2icns with multiple PNG sizes
    echo "Generating macOS ICNS (using png2icns)..."
    
    # Create temp directory for PNGs
    TEMP_DIR=$(mktemp -d)
    trap "rm -rf $TEMP_DIR" EXIT
    
    # Generate all required sizes (png2icns doesn't support 64x64)
    "$CONVERT_CMD" -background none "$SVG_SOURCE" -resize 16x16     "${TEMP_DIR}/icon_16.png"
    "$CONVERT_CMD" -background none "$SVG_SOURCE" -resize 32x32     "${TEMP_DIR}/icon_32.png"
    "$CONVERT_CMD" -background none "$SVG_SOURCE" -resize 128x128   "${TEMP_DIR}/icon_128.png"
    "$CONVERT_CMD" -background none "$SVG_SOURCE" -resize 256x256   "${TEMP_DIR}/icon_256.png"
    "$CONVERT_CMD" -background none "$SVG_SOURCE" -resize 512x512   "${TEMP_DIR}/icon_512.png"
    "$CONVERT_CMD" -background none "$SVG_SOURCE" -resize 1024x1024 "${TEMP_DIR}/icon_1024.png"
    
    # png2icns automatically assigns sizes based on image dimensions
    # Suppress JasPer library deprecation warnings
    png2icns "${MACOS_DIR}/nexus.icns" \
        "${TEMP_DIR}/icon_16.png" \
        "${TEMP_DIR}/icon_32.png" \
        "${TEMP_DIR}/icon_128.png" \
        "${TEMP_DIR}/icon_256.png" \
        "${TEMP_DIR}/icon_512.png" \
        "${TEMP_DIR}/icon_1024.png" \
        2>/dev/null
    
    echo "✓ nexus.icns (with all sizes: 16, 32, 128, 256, 512, 1024)"
fi

echo ""
echo "✓ macOS asset generation complete!"