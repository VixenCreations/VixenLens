@echo off
SETLOCAL EnableDelayedExpansion
TITLE VixenLens Deep Clean ^& Build Pipeline

cd /d "%~dp0"
set "LOGFILE=%~dp0build.log"
set "RELEASE_DIR=%~dp0Built-Release"

:: Initialize Log
echo ======================================== > "%LOGFILE%"
echo VixenLens Deep Clean ^& Build Pipeline Started >> "%LOGFILE%"
echo %DATE% %TIME% >> "%LOGFILE%"
echo ======================================== >> "%LOGFILE%"

echo [1/5] Performing deep clean of all cache environments...
echo [1/5] Performing deep clean of all cache environments... >> "%LOGFILE%"

:: 1. Nuke Rust Cache
if exist "src-tauri\target" (
    echo   - Wiping Rust backend target cache...
    echo   - Wiping Rust backend target cache... >> "%LOGFILE%"
    rmdir /s /q "src-tauri\target"
)

:: 2. Nuke SvelteKit/Vite Caches
if exist ".svelte-kit" (
    echo   - Wiping SvelteKit intermediate cache...
    echo   - Wiping SvelteKit intermediate cache... >> "%LOGFILE%"
    rmdir /s /q ".svelte-kit"
)
if exist "build" (
    echo   - Wiping frontend output directory...
    echo   - Wiping frontend output directory... >> "%LOGFILE%"
    rmdir /s /q "build"
)

:: 3. Purge Release Directory
if exist "%RELEASE_DIR%" (
    echo   - Purging previous release artifacts...
    echo   - Purging previous release artifacts... >> "%LOGFILE%"
    del /q /f "%RELEASE_DIR%\*.*" >nul 2>&1
)

echo.
echo [2/5] Validating Frontend Dependencies...
echo [2/5] Validating Frontend Dependencies... >> "%LOGFILE%"
call npm install >> "%LOGFILE%" 2>&1

echo.
echo [3/5] Executing Tauri Production Build (This will take longer due to cold cache)...
echo [3/5] Executing Tauri Production Build (Cold Cache)... >> "%LOGFILE%"
call npm run tauri build >> "%LOGFILE%" 2>&1

echo.
echo [4/5] Locating Executable...
echo [4/5] Locating Executable... >> "%LOGFILE%"
set "FOUND="

if exist "src-tauri\target\release\vixen-lens.exe" set "FOUND=src-tauri\target\release\vixen-lens.exe"

if defined FOUND (
    echo ----------------------------------------------------------
    echo BUILD SUCCESSFUL
    echo Binary: !FOUND!
    echo ----------------------------------------------------------
    echo. >> "%LOGFILE%"
    echo BUILD SUCCESSFUL >> "%LOGFILE%"
    echo Binary: !FOUND! >> "%LOGFILE%"

    echo.
    echo [5/5] Packaging to Built-Release directory...
    echo [5/5] Packaging to Built-Release directory... >> "%LOGFILE%"

    :: Create release directory safely
    if not exist "%RELEASE_DIR%" mkdir "%RELEASE_DIR%"

    :: Copy Executable
    copy /y "!FOUND!" "%RELEASE_DIR%\" >nul
    if !errorlevel! equ 0 (
        echo   - Copied Executable >> "%LOGFILE%"
        echo   - Copied Executable
    ) else (
        echo   [!] Failed to copy Executable >> "%LOGFILE%"
        echo   [!] Failed to copy Executable
    )

    :: Copy MSI Installer
    if exist "src-tauri\target\release\bundle\msi\*.msi" (
        copy /y "src-tauri\target\release\bundle\msi\*.msi" "%RELEASE_DIR%\" >nul
        if !errorlevel! equ 0 (
            echo   - Copied MSI Installer >> "%LOGFILE%"
            echo   - Copied MSI Installer
        ) else (
            echo   [!] Failed to copy MSI Installer >> "%LOGFILE%"
            echo   [!] Failed to copy MSI Installer
        )
    )

    :: Copy NSIS Setup
    if exist "src-tauri\target\release\bundle\nsis\*-setup.exe" (
        copy /y "src-tauri\target\release\bundle\nsis\*-setup.exe" "%RELEASE_DIR%\" >nul
        if !errorlevel! equ 0 (
            echo   - Copied NSIS Installer >> "%LOGFILE%"
            echo   - Copied NSIS Installer
        ) else (
            echo   [!] Failed to copy NSIS Installer >> "%LOGFILE%"
            echo   [!] Failed to copy NSIS Installer
        )
    )

    echo.
    echo Release files packaged securely!
    echo Release files packaged securely! >> "%LOGFILE%"
    echo Details saved to Built-Release\build.log

) else (
    echo.
    echo [!] Build failed. Executable not found.
    echo Please check build.log for detailed Rust or NodeJS errors.
    echo. >> "%LOGFILE%"
    echo BUILD FAILED >> "%LOGFILE%"
)

:: Copy log to release folder as the absolute last step
if exist "%RELEASE_DIR%" (
    copy /y "%LOGFILE%" "%RELEASE_DIR%\build.log" >nul
)

pause
ENDLOCAL