#!/usr/bin/env bash
# Builds the Linux tarball (x64). Mirrors build-windows.bat.
# Needs: python 3.11+, node 22+ (with npm), Rust (rustup), the WebKitGTK development files, and a C
# toolchain to build ffmpeg (about 5 minutes the first time):
#   Arch / CachyOS: sudo pacman -S python nodejs npm rustup base-devel nasm cmake openssl curl webkit2gtk-4.1 libayatana-appindicator librsvg
#   Debian / Ubuntu: sudo apt install build-essential nasm cmake pkg-config libssl-dev curl python3-venv \
#                      libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
# Output: dist/app/PyxeeRadio-<version>-linux-x64.tar.gz
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

echo "=== 1/5  Python environment ==="
[ -d .venv ] || python3 -m venv .venv
.venv/bin/python -m pip install --disable-pip-version-check --require-hashes -r requirements-build.lock
.venv/bin/python -c "import yt_dlp.version as v; print('[ok] yt-dlp', v.__version__)"

echo "=== 2/5  ffmpeg (small build from pinned, checksum-verified source) and QuickJS ==="
if [ -x packaging/bin/ffmpeg ]; then
  echo "[ok] packaging/bin/ffmpeg already exists (delete packaging/bin to rebuild it)"
else
  ./packaging/ffmpeg/build.sh linux packaging/bin
fi
.venv/bin/python scripts/check_ffmpeg.py packaging/bin/ffmpeg
# yt-dlp's JavaScript runtime in the installed app (about 2 MB; pinned and checksum-verified)
.venv/bin/python scripts/fetch_qjs.py linux

echo "=== 3/5  Bot core (PyInstaller) ==="
rm -rf dist/core
.venv/bin/pyinstaller packaging/core.spec --noconfirm --distpath dist/core --workpath build/pyinstaller
test -x dist/core/PyxeeRadioCore/PyxeeRadioCore

echo "=== 4/5  Desktop app: page checks ==="
(cd app && npm ci && npm run check && npm test)

echo "=== 5/5  Desktop app: build and package ==="
(cd app && npx tauri build --no-bundle)
version="$(.venv/bin/python -c 'from twitch_radio.version import APP_VERSION; print(APP_VERSION)')"
scripts/package-linux.sh "$version"

echo "Done. The tarball is in dist/app/"
