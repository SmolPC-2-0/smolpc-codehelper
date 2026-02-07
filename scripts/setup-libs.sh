#!/usr/bin/env bash
# Download ONNX Runtime for the current platform.
# Extracts the shared library to src-tauri/libs/.
#
# Version notes:
#   - Windows uses v1.22.1 (latest, Windows-only patch release)
#   - macOS/Linux use v1.22.0 (latest with cross-platform binaries)
#
# Usage:
#   ./scripts/setup-libs.sh              # Auto-detect platform
#   ./scripts/setup-libs.sh --platform windows-x64
#   ./scripts/setup-libs.sh --platform macos-arm64
#   ./scripts/setup-libs.sh --platform linux-x64

set -euo pipefail

ORT_VERSION_WINDOWS="1.22.1"
ORT_VERSION_DEFAULT="1.22.0"
LIBS_DIR="src-tauri/libs"

# --- Platform detection ---

detect_platform() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Darwin)
            case "$arch" in
                arm64) echo "macos-arm64" ;;
                x86_64) echo "macos-x64" ;;
                *) echo "Unsupported macOS architecture: $arch" >&2; exit 1 ;;
            esac
            ;;
        Linux)
            case "$arch" in
                x86_64) echo "linux-x64" ;;
                aarch64) echo "linux-arm64" ;;
                *) echo "Unsupported Linux architecture: $arch" >&2; exit 1 ;;
            esac
            ;;
        MINGW*|MSYS*|CYGWIN*)
            echo "windows-x64"
            ;;
        *)
            echo "Unsupported OS: $os" >&2
            exit 1
            ;;
    esac
}

# Parse args
PLATFORM=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --platform)
            PLATFORM="$2"
            shift 2
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

if [[ -z "$PLATFORM" ]]; then
    PLATFORM="$(detect_platform)"
fi

# --- Map platform to download URL and library filename ---

case "$PLATFORM" in
    windows-x64)
        ORT_VERSION="$ORT_VERSION_WINDOWS"
        ARCHIVE_NAME="onnxruntime-win-x64-${ORT_VERSION}.zip"
        DYLIB_NAME="onnxruntime.dll"
        INNER_DIR="onnxruntime-win-x64-${ORT_VERSION}"
        ;;
    macos-arm64)
        ORT_VERSION="$ORT_VERSION_DEFAULT"
        ARCHIVE_NAME="onnxruntime-osx-arm64-${ORT_VERSION}.tgz"
        DYLIB_NAME="libonnxruntime.dylib"
        INNER_DIR="onnxruntime-osx-arm64-${ORT_VERSION}"
        ;;
    macos-x64)
        ORT_VERSION="$ORT_VERSION_DEFAULT"
        ARCHIVE_NAME="onnxruntime-osx-x86_64-${ORT_VERSION}.tgz"
        DYLIB_NAME="libonnxruntime.dylib"
        INNER_DIR="onnxruntime-osx-x86_64-${ORT_VERSION}"
        ;;
    linux-x64)
        ORT_VERSION="$ORT_VERSION_DEFAULT"
        ARCHIVE_NAME="onnxruntime-linux-x64-${ORT_VERSION}.tgz"
        DYLIB_NAME="libonnxruntime.so"
        INNER_DIR="onnxruntime-linux-x64-${ORT_VERSION}"
        ;;
    linux-arm64)
        ORT_VERSION="$ORT_VERSION_DEFAULT"
        ARCHIVE_NAME="onnxruntime-linux-aarch64-${ORT_VERSION}.tgz"
        DYLIB_NAME="libonnxruntime.so"
        INNER_DIR="onnxruntime-linux-aarch64-${ORT_VERSION}"
        ;;
    *)
        echo "Unknown platform: $PLATFORM" >&2
        exit 1
        ;;
esac

echo "Platform: $PLATFORM"
echo "ONNX Runtime version: $ORT_VERSION"

BASE_URL="https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}"

DOWNLOAD_URL="${BASE_URL}/${ARCHIVE_NAME}"
TARGET_PATH="${LIBS_DIR}/${DYLIB_NAME}"

# --- Idempotency check ---

if [[ -f "$TARGET_PATH" ]]; then
    echo "Library already exists at $TARGET_PATH — skipping download."
    exit 0
fi

# --- Download and extract ---

mkdir -p "$LIBS_DIR"

TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TEMP_DIR"' EXIT

echo "Downloading $DOWNLOAD_URL ..."
curl -fSL --progress-bar -o "$TEMP_DIR/$ARCHIVE_NAME" "$DOWNLOAD_URL"

echo "Extracting $DYLIB_NAME ..."

case "$ARCHIVE_NAME" in
    *.zip)
        # Windows: zip archive — extract just the lib file
        unzip -q -o "$TEMP_DIR/$ARCHIVE_NAME" "${INNER_DIR}/lib/${DYLIB_NAME}" -d "$TEMP_DIR"
        ;;
    *.tgz)
        # macOS/Linux: tar.gz archive — extract the full archive then pick the file
        tar -xzf "$TEMP_DIR/$ARCHIVE_NAME" -C "$TEMP_DIR"
        ;;
esac

# Copy just the library file we need
if [[ ! -f "$TEMP_DIR/${INNER_DIR}/lib/${DYLIB_NAME}" ]]; then
    echo "ERROR: Expected file not found in archive: ${INNER_DIR}/lib/${DYLIB_NAME}" >&2
    echo "Archive contents:" >&2
    ls -la "$TEMP_DIR/${INNER_DIR}/lib/" >&2 2>/dev/null || true
    exit 1
fi
cp "$TEMP_DIR/${INNER_DIR}/lib/${DYLIB_NAME}" "$TARGET_PATH"

echo "Installed $DYLIB_NAME to $TARGET_PATH"
echo "Done."
