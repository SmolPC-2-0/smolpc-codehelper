@echo off
echo ========================================
echo Building Blender Learning Assistant
echo ========================================
echo.

REM 1. Install frontend dependencies
echo [1/3] Installing frontend dependencies...
call npm install
if %errorlevel% neq 0 (
    echo ERROR: Failed to install frontend dependencies
    pause
    exit /b 1
)
echo.

REM 2. Build frontend
echo [2/3] Building frontend...
call npm run build
if %errorlevel% neq 0 (
    echo ERROR: Failed to build frontend
    pause
    exit /b 1
)
echo.

REM 3. Build Tauri app
echo [3/3] Building Tauri app...
call npm run tauri build
if %errorlevel% neq 0 (
    echo ERROR: Failed to build Tauri app
    pause
    exit /b 1
)
echo.

echo ========================================
echo Build complete!
echo ========================================
echo.
echo Installer location:
echo   src-tauri\target\release\bundle\msi\
echo.
echo Executable location:
echo   src-tauri\target\release\blender_helper.exe
echo.
pause
