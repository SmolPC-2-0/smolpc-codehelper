#!/usr/bin/env bash
# Download ONNX Runtime shared libraries for the current platform.
#
# Windows uses the DirectML runtime package so we can bundle:
# - onnxruntime.dll
# - onnxruntime_providers_shared.dll
# - DirectML.dll
#
# macOS/Linux use official ONNX Runtime release archives and bundle:
# - libonnxruntime.dylib / libonnxruntime.so
#
# Usage:
#   ./scripts/setup-libs.sh
#   ./scripts/setup-libs.sh --platform windows-x64
#   ./scripts/setup-libs.sh --platform windows-arm64
#   ./scripts/setup-libs.sh --platform macos-arm64
#   ./scripts/setup-libs.sh --platform macos-x64
#   ./scripts/setup-libs.sh --platform linux-x64
#   ./scripts/setup-libs.sh --platform linux-arm64
#   ./scripts/setup-libs.sh --force

set -euo pipefail

ORT_VERSION="1.23.0"
DML_ORT_PACKAGE_VERSION="1.23.0"
DML_PACKAGE_VERSION="1.15.4"
LIBS_DIR="src-tauri/libs"

# SHA256 checksums
SHA_DML_ORT_NUPKG="a33ec2382b3c440bab74042a135733bb6e5085f293b908d3997688a58fe307e7"
SHA_DML_NUPKG="4e7cb7ddce8cf837a7a75dc029209b520ca0101470fcdf275c1f49736a3615b9"
SHA_ORT_MACOS_ARM64="8182db0ebb5caa21036a3c78178f17fabb98a7916bdab454467c8f4cf34bcfdf"
SHA_ORT_MACOS_X64="a8e43edcaa349cbfc51578a7fc61ea2b88793ccf077b4bc65aca58999d20cf0f"
SHA_ORT_LINUX_X64="b6deea7f2e22c10c043019f294a0ea4d2a6c0ae52a009c34847640db75ec5580"
SHA_ORT_LINUX_ARM64="0b9f47d140411d938e47915824d8daaa424df95a88b5f1fc843172a75168f7a0"

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
                aarch64|arm64) echo "linux-arm64" ;;
                *) echo "Unsupported Linux architecture: $arch" >&2; exit 1 ;;
            esac
            ;;
        MINGW*|MSYS*|CYGWIN*)
            case "$arch" in
                x86_64) echo "windows-x64" ;;
                aarch64|arm64) echo "windows-arm64" ;;
                *) echo "Unsupported Windows architecture: $arch" >&2; exit 1 ;;
            esac
            ;;
        *)
            echo "Unsupported OS: $os" >&2
            exit 1
            ;;
    esac
}

sha256_file() {
    local file="$1"
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$file" | awk '{print $1}'
    elif command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$file" | awk '{print $1}'
    else
        echo "Neither shasum nor sha256sum is available" >&2
        exit 1
    fi
}

download_with_checksum() {
    local url="$1"
    local out="$2"
    local expected_sha="$3"

    echo "Downloading $url"
    curl -fSL --progress-bar -o "$out" "$url"

    local actual_sha
    actual_sha="$(sha256_file "$out")"
    if [[ "$actual_sha" != "$expected_sha" ]]; then
        echo "Checksum mismatch for $out" >&2
        echo "Expected: $expected_sha" >&2
        echo "Actual:   $actual_sha" >&2
        exit 1
    fi
}

copy_required_file() {
    local from="$1"
    local to="$2"

    if [[ ! -f "$from" ]]; then
        echo "Required file not found: $from" >&2
        exit 1
    fi
    cp "$from" "$to"
}

PLATFORM=""
FORCE="false"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --platform)
            PLATFORM="$2"
            shift 2
            ;;
        --force)
            FORCE="true"
            shift
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

mkdir -p "$LIBS_DIR"

required_files=()
case "$PLATFORM" in
    windows-x64)
        required_files=("onnxruntime.dll" "onnxruntime_providers_shared.dll" "DirectML.dll")
        ;;
    windows-arm64)
        required_files=("onnxruntime.dll" "onnxruntime_providers_shared.dll" "DirectML.dll")
        ;;
    macos-arm64|macos-x64)
        required_files=("libonnxruntime.dylib")
        ;;
    linux-x64|linux-arm64)
        required_files=("libonnxruntime.so")
        ;;
    *)
        echo "Unknown platform: $PLATFORM" >&2
        exit 1
        ;;
esac

if [[ "$FORCE" == "true" ]]; then
    for f in "${required_files[@]}"; do
        rm -f "$LIBS_DIR/$f"
    done
fi

all_present="true"
for f in "${required_files[@]}"; do
    if [[ ! -f "$LIBS_DIR/$f" ]]; then
        all_present="false"
        break
    fi
done

if [[ "$all_present" == "true" ]]; then
    echo "All required runtime files already exist in $LIBS_DIR. Use --force to re-install."
    exit 0
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

echo "Platform: $PLATFORM"
echo "ORT runtime target: $ORT_VERSION"

install_windows() {
    local runtime_arch="$1"
    local dml_arch="$2"

    local ort_pkg_name="microsoft.ml.onnxruntime.directml.${DML_ORT_PACKAGE_VERSION}.nupkg"
    local ort_pkg_url="https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime.directml/${DML_ORT_PACKAGE_VERSION}/${ort_pkg_name}"
    local dml_pkg_name="microsoft.ai.directml.${DML_PACKAGE_VERSION}.nupkg"
    local dml_pkg_url="https://api.nuget.org/v3-flatcontainer/microsoft.ai.directml/${DML_PACKAGE_VERSION}/${dml_pkg_name}"

    download_with_checksum "$ort_pkg_url" "$tmp_dir/$ort_pkg_name" "$SHA_DML_ORT_NUPKG"
    unzip -q -o "$tmp_dir/$ort_pkg_name" -d "$tmp_dir/ort-pkg"

    copy_required_file "$tmp_dir/ort-pkg/runtimes/${runtime_arch}/native/onnxruntime.dll" "$LIBS_DIR/onnxruntime.dll"
    copy_required_file "$tmp_dir/ort-pkg/runtimes/${runtime_arch}/native/onnxruntime_providers_shared.dll" "$LIBS_DIR/onnxruntime_providers_shared.dll"

    download_with_checksum "$dml_pkg_url" "$tmp_dir/$dml_pkg_name" "$SHA_DML_NUPKG"
    unzip -q -o "$tmp_dir/$dml_pkg_name" -d "$tmp_dir/dml-pkg"

    copy_required_file "$tmp_dir/dml-pkg/bin/${dml_arch}/DirectML.dll" "$LIBS_DIR/DirectML.dll"
}

install_ort_archive() {
    local archive_name="$1"
    local inner_dir="$2"
    local dylib_name="$3"
    local checksum="$4"

    local url="https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/${archive_name}"

    download_with_checksum "$url" "$tmp_dir/$archive_name" "$checksum"
    tar -xzf "$tmp_dir/$archive_name" -C "$tmp_dir"
    copy_required_file "$tmp_dir/${inner_dir}/lib/${dylib_name}" "$LIBS_DIR/${dylib_name}"
}

case "$PLATFORM" in
    windows-x64)
        install_windows "win-x64" "x64-win"
        ;;
    windows-arm64)
        install_windows "win-arm64" "arm64-win"
        ;;
    macos-arm64)
        install_ort_archive \
            "onnxruntime-osx-arm64-${ORT_VERSION}.tgz" \
            "onnxruntime-osx-arm64-${ORT_VERSION}" \
            "libonnxruntime.dylib" \
            "$SHA_ORT_MACOS_ARM64"
        ;;
    macos-x64)
        install_ort_archive \
            "onnxruntime-osx-x86_64-${ORT_VERSION}.tgz" \
            "onnxruntime-osx-x86_64-${ORT_VERSION}" \
            "libonnxruntime.dylib" \
            "$SHA_ORT_MACOS_X64"
        ;;
    linux-x64)
        install_ort_archive \
            "onnxruntime-linux-x64-${ORT_VERSION}.tgz" \
            "onnxruntime-linux-x64-${ORT_VERSION}" \
            "libonnxruntime.so" \
            "$SHA_ORT_LINUX_X64"
        ;;
    linux-arm64)
        install_ort_archive \
            "onnxruntime-linux-aarch64-${ORT_VERSION}.tgz" \
            "onnxruntime-linux-aarch64-${ORT_VERSION}" \
            "libonnxruntime.so" \
            "$SHA_ORT_LINUX_ARM64"
        ;;
    *)
        echo "Unknown platform: $PLATFORM" >&2
        exit 1
        ;;
esac

echo "Installed runtime libraries into $LIBS_DIR"
ls -la "$LIBS_DIR"
echo "Done."
