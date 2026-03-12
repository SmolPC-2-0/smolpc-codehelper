#!/bin/bash

echo "========================================"
echo "Building Blender Learning Assistant"
echo "========================================"
echo ""

# 1. Install frontend dependencies
echo "[1/3] Installing frontend dependencies..."
npm install
if [ $? -ne 0 ]; then
    echo "ERROR: Failed to install frontend dependencies"
    exit 1
fi
echo ""

# 2. Build frontend
echo "[2/3] Building frontend..."
npm run build
if [ $? -ne 0 ]; then
    echo "ERROR: Failed to build frontend"
    exit 1
fi
echo ""

# 3. Build Tauri app
echo "[3/3] Building Tauri app..."
npm run tauri build
if [ $? -ne 0 ]; then
    echo "ERROR: Failed to build Tauri app"
    exit 1
fi
echo ""

echo "========================================"
echo "Build complete!"
echo "========================================"
echo ""
echo "Build artifacts location:"
echo "  src-tauri/target/release/bundle/"
echo ""
