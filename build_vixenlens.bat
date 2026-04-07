@echo off
SETLOCAL
TITLE VixenLens Build Pipeline

cd /d "%~dp0"
set LOGFILE="%~dp0build.log"

:: Initialize Log
echo ======================================== > %LOGFILE%
echo VixenLens Build Pipeline Started >> %LOGFILE%
echo %DATE% %TIME% >> %LOGFILE%
echo ======================================== >> %LOGFILE%

echo [1/4] Cleaning previous build artifacts...
echo [1/4] Cleaning previous build artifacts... >> %LOGFILE%

echo [2/4] Validating Frontend Dependencies...
echo [2/4] Validating Frontend Dependencies... >> %LOGFILE%
call npm install >> %LOGFILE% 2>&1

echo [3/4] Executing Tauri Production Build (This may take a minute)...
echo [3/4] Executing Tauri Production Build... >> %LOGFILE%
call npm run tauri build >> %LOGFILE% 2>&1

echo [4/4] Locating Executable...
echo [4/4] Locating Executable... >> %LOGFILE%
set "FOUND="

:: Tauri uses the productName from tauri.conf.json for the final executable
if exist "src-tauri\target\release\vixen-lens.exe" set "FOUND=src-tauri\target\release\vixen-lens.exe"

if defined FOUND (
    echo.
    echo ----------------------------------------------------------
    echo BUILD SUCCESSFUL
    echo Binary: %FOUND%
    echo Details saved to build.log
    echo ----------------------------------------------------------
    echo. >> %LOGFILE%
    echo BUILD SUCCESSFUL >> %LOGFILE%
    echo Binary: %FOUND% >> %LOGFILE%
) else (
    echo.
    echo [!] Build failed. Executable not found.
    echo Please check build.log for detailed Rust or NodeJS errors.
    echo. >> %LOGFILE%
    echo BUILD FAILED >> %LOGFILE%
)

pause
ENDLOCAL