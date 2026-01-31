#!/bin/sh
# Generate macOS application assets from SVG source
# Requires: ImageMagick (magick/convert), macOS iconutil
#
# NOTE: This script must be run on macOS. The iconutil command is required
# to generate ICNS files in the modern "ic10" format that displays correctly
# in macOS app bundles. Linux alternatives like png2icns produce older formats
# that don't work properly.

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
    echo "Install with: brew install imagemagick" >&2
    exit 1
fi

# Determine which ImageMagick command to use
if command -v magick >/dev/null 2>&1; then
    CONVERT_CMD="magick"
else
    CONVERT_CMD="convert"
fi

# Require iconutil (macOS only)
if ! command -v iconutil >/dev/null 2>&1; then
    echo "Error: iconutil not found - this script must be run on macOS" >&2
    echo "iconutil is required to generate ICNS files in the correct format" >&2
    exit 1
fi

echo "Generating macOS assets from $SVG_SOURCE"
echo ""

# Generate macOS PNG (1024x1024) with transparency
echo "Generating PNG (1024x1024)..."
"$CONVERT_CMD" -background none "$SVG_SOURCE" -resize 1024x1024 "${MACOS_DIR}/nexus.png"
echo "✓ nexus.png"

# Generate ICNS with all required sizes using iconutil
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

echo ""
echo "✓ macOS asset generation complete!"