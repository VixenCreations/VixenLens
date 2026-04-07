@echo off
SETLOCAL
TITLE VixenLens Build Pipeline

cd /d "%~dp0"
set LOGFILE="%~dp0build.log"
set RELEASE_DIR="%~dp0Built-Release"

:: Initialize Log
echo ======================================== > %LOGFILE%
echo VixenLens Build Pipeline Started >> %LOGFILE%
echo %DATE% %TIME% >> %LOGFILE%
echo ======================================== >> %LOGFILE%

echo [1/5] Cleaning previous build artifacts...
echo [1/5] Cleaning previous build artifacts... >> %LOGFILE%

echo [2/5] Validating Frontend Dependencies...
echo [2/5] Validating Frontend Dependencies... >> %LOGFILE%
call npm install >> %LOGFILE% 2>&1

echo [3/5] Executing Tauri Production Build (This may take a minute)...
echo [3/5] Executing Tauri Production Build... >> %LOGFILE%
call npm run tauri build >> %LOGFILE% 2>&1

echo [4/5] Locating Executable...
echo [4/5] Locating Executable... >> %LOGFILE%
set "FOUND="

:: Tauri uses the package name from Cargo.toml for the final executable
if exist "src-tauri\target\release\vixen-lens.exe" set "FOUND=src-tauri\target\release\vixen-lens.exe"

if defined FOUND (
    echo.
    echo ----------------------------------------------------------
    echo BUILD SUCCESSFUL
    echo Binary: %FOUND%
    echo ----------------------------------------------------------
    echo. >> %LOGFILE%
    echo BUILD SUCCESSFUL >> %LOGFILE%
    echo Binary: %FOUND% >> %LOGFILE%

    echo.
    echo [5/5] Packaging to Built-Release directory...
    echo [5/5] Packaging to Built-Release directory... >> %LOGFILE%

    :: Create release directory if it doesn't exist
    if not exist "%RELEASE_DIR%" mkdir "%RELEASE_DIR%"

    :: Copy Executable
    copy /y "%FOUND%" "%RELEASE_DIR%\" >nul
    echo   - Copied Executable >> %LOGFILE%
    echo   - Copied Executable

    :: Copy MSI Installer using wildcards to future-proof version bumps
    if exist "src-tauri\target\release\bundle\msi\*.msi" (
        copy /y "src-tauri\target\release\bundle\msi\*.msi" "%RELEASE_DIR%\" >nul
        echo   - Copied MSI Installer >> %LOGFILE%
        echo   - Copied MSI Installer
    )

    :: Copy NSIS Setup using wildcards to future-proof version bumps
    if exist "src-tauri\target\release\bundle\nsis\*-setup.exe" (
        copy /y "src-tauri\target\release\bundle\nsis\*-setup.exe" "%RELEASE_DIR%\" >nul
        echo   - Copied NSIS Installer >> %LOGFILE%
        echo   - Copied NSIS Installer
    )

    echo.
    echo Release files packaged successfully!
    echo Release files packaged successfully! >> %LOGFILE%
    echo Details saved to Built-Release\build.log

) else (
    echo.
    echo [!] Build failed. Executable not found.
    echo Please check build.log for detailed Rust or NodeJS errors.
    echo. >> %LOGFILE%
    echo BUILD FAILED >> %LOGFILE%
)

:: Copy log to release folder as the absolute last step so it includes the packaging logs
if exist "%RELEASE_DIR%" (
    copy /y %LOGFILE% "%RELEASE_DIR%\build.log" >nul
)

pause
ENDLOCAL