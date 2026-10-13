@echo off
setlocal EnableExtensions
cd /d "%~dp0"
rem Builds the installer: app\src-tauri\target\release\bundle\nsis\PyxeeRadio_x.y.z_x64-setup.exe
rem Needs: Python 3.11+, Node.js 22+ (npm), Rust (https://rustup.rs, with the Visual Studio C++ Build Tools), and packaging\bin\ffmpeg.exe, the small ffmpeg that CI builds from pinned source (see BUILD.md for how to get it). A random ffmpeg from PATH is never bundled.

set "BIN=%~dp0packaging\bin"

echo.
echo === Checking prerequisites ===

where python >nul 2>nul || (echo [missing] Python was not found on PATH. Install 3.11 or newer from https://www.python.org/downloads/ & goto :fail)
python -c "import sys; sys.exit(0 if sys.version_info >= (3, 11) else 1)" >nul 2>nul || (echo [problem] Python 3.11 or newer is required. If python is the Microsoft Store shortcut, install the real one. & goto :fail)
for /f "delims=" %%v in ('python --version 2^>^&1') do echo [ok] %%v

where npm >nul 2>nul || (echo [missing] Node.js/npm was not found on PATH. Install Node.js LTS from https://nodejs.org & goto :fail)
node -e "process.exit(Number(process.versions.node.split('.')[0]) >= 22 ? 0 : 1)" >nul 2>nul || (echo [problem] Node.js 22 or newer is required. & goto :fail)
for /f "delims=" %%v in ('node --version 2^>^&1') do echo [ok] Node.js %%v

where cargo >nul 2>nul || (echo [missing] Rust was not found on PATH. Install it from https://rustup.rs ^(it also asks for the Visual Studio C++ Build Tools^) & goto :fail)
for /f "delims=" %%v in ('cargo --version 2^>^&1') do echo [ok] %%v

echo.
echo === 1/5  Python environment ===
if exist ".venv\Scripts\python.exe" (echo [ok] .venv already exists) else (python -m venv .venv || goto :fail)
call .venv\Scripts\activate.bat
python -m pip install --disable-pip-version-check --require-hashes -r requirements-build.lock || goto :fail
python -c "import yt_dlp.version as v; print('[ok] yt-dlp', v.__version__)" || goto :fail

echo.
echo === 2/5  ffmpeg and QuickJS ===
if not exist "%BIN%" mkdir "%BIN%"
rem Older builds put deno.exe here; anything in bin\ is bundled, so drop the leftover.
if exist "%BIN%\deno.exe" del /q "%BIN%\deno.exe"
if not exist "%BIN%\ffmpeg.exe" goto :no_ffmpeg
python scripts\check_ffmpeg.py "%BIN%\ffmpeg.exe" || goto :fail
goto :tools_ready
:no_ffmpeg
echo [missing] %BIN%\ffmpeg.exe
echo.
echo The installer ships a small ffmpeg that is built from pinned source, not a general-purpose download.
echo Get it either way:
echo   a^) From CI: open the latest "CI" or "Release" run on GitHub, download the artifact
echo      "ffmpeg-windows-x64", and unzip it into packaging\bin\ ^(ffmpeg.exe + FFMPEG-LICENSE.txt^).
echo   b^) Build it yourself in WSL or any Linux:   bash packaging/ffmpeg/build.sh windows packaging/bin
goto :fail
:tools_ready
rem QuickJS is yt-dlp's JavaScript runtime in the installed app (about 2 MB; pinned and checksum-verified).
python scripts\fetch_qjs.py windows || goto :fail

echo.
echo === 3/5  Bot core (PyInstaller) ===
if exist dist\core rmdir /s /q dist\core
pyinstaller packaging\core.spec --noconfirm --distpath dist\core --workpath build\pyinstaller || goto :fail
if not exist dist\core\PyxeeRadioCore\PyxeeRadioCore.exe (echo Core build did not produce PyxeeRadioCore.exe & goto :fail)

echo.
echo === 4/5  Desktop app: page checks ===
pushd app
call npm ci || (popd & goto :fail)
call npm run check || (popd & goto :fail)
call npm test || (popd & goto :fail)

echo.
echo === 5/5  Desktop app: installer (Tauri) ===
call npx tauri build --bundles nsis --config src-tauri/tauri.release.conf.json || (popd & goto :fail)
popd

echo.
echo Done. Installer is in app\src-tauri\target\release\bundle\nsis\
exit /b 0

:fail
echo.
echo BUILD FAILED - see the messages above.
pause
exit /b 1
