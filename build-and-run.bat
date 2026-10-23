@echo off
setlocal EnableExtensions DisableDelayedExpansion
title Achimeld - Build and Run
set "pause_at_end=0"
if "%~1"=="" set "pause_at_end=1"
set "build_options="

:arguments
if "%~1"=="" goto find_powershell
if /i "%~1"=="--help" goto usage
if /i "%~1"=="-h" goto usage
if /i "%~1"=="--offline" (
    set "build_options=%build_options% -Offline"
    shift
    goto arguments
)
if /i "%~1"=="--build-only" (
    set "build_options=%build_options% -BuildOnly"
    shift
    goto arguments
)
if /i "%~1"=="--protected" (
    set "build_options=%build_options% -Protected"
    shift
    goto arguments
)
echo Unsupported option. Use --help for usage.
set "run_exit=1"
goto finish

:find_powershell
set "powershell_path="
where pwsh.exe >nul 2>&1
if not errorlevel 1 set "powershell_path=pwsh.exe"
if not defined powershell_path if exist "%ProgramW6432%\PowerShell\7\pwsh.exe" set "powershell_path=%ProgramW6432%\PowerShell\7\pwsh.exe"
if not defined powershell_path if exist "%ProgramFiles%\PowerShell\7\pwsh.exe" set "powershell_path=%ProgramFiles%\PowerShell\7\pwsh.exe"
if not defined powershell_path if exist "%LOCALAPPDATA%\Microsoft\PowerShell\7\pwsh.exe" set "powershell_path=%LOCALAPPDATA%\Microsoft\PowerShell\7\pwsh.exe"
if not defined powershell_path if exist "%USERPROFILE%\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe" set "powershell_path=%USERPROFILE%\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe"
if not defined powershell_path (
    echo PowerShell 7 was not found. Install it, then run this file again.
    set "run_exit=1"
    goto finish
)

pushd "%~dp0"
if errorlevel 1 (
    echo Could not open the repository directory.
    set "run_exit=1"
    goto finish
)
"%powershell_path%" -NoLogo -NoProfile -File "%~dp0build\Build-And-Run.ps1" %build_options%
set "run_exit=%errorlevel%"
popd

:finish
if "%pause_at_end%"=="1" pause
exit /b %run_exit%

:usage
echo Usage: build-and-run.bat [--offline] [--build-only] [--protected]
echo.
echo Builds and launches Achimeld as the current user, without administrator approval.
echo Cleans temporary artifacts only after a successful build; keeps reusable outputs.
echo --offline uses cached dependencies and the verified browser archive.
echo --build-only builds and verifies the package without launching.
echo --protected optionally installs Windows network isolation with administrator approval.
exit /b 0
