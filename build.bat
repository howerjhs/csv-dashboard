@echo off
rem Release build, then copy the exe next to this script.
rem MinGW dlltool cannot handle non-ASCII paths, so build outside this folder.
setlocal
for /d %%D in ("%LOCALAPPDATA%\Microsoft\WinGet\Packages\BrechtSanders.WinLibs*") do set "PATH=%%D\mingw64\bin;%PATH%"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
if not defined CARGO_TARGET_DIR set "CARGO_TARGET_DIR=%USERPROFILE%\.cargo\targets\csv_dashboard"
cd /d "%~dp0"
cargo build --release || exit /b 1
copy /y "%CARGO_TARGET_DIR%\release\csv_dashboard.exe" "%~dp0CSV_Dashboard.exe" >nul
echo Done: CSV_Dashboard.exe
