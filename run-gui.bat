@echo off
setlocal
cd /d "%~dp0"
rem Developer shortcut: runs the desktop app against your source checkout
rem (uses .venv and your existing .env/data). Needs Node.js, Rust and `npm install` once in app\.
where npm >nul 2>nul || (echo Node.js/npm was not found on PATH. & pause & exit /b 1)
where cargo >nul 2>nul || (echo Rust was not found on PATH. Install it from https://rustup.rs & pause & exit /b 1)
pushd app
if not exist node_modules call npm install
call npx tauri dev
popd
