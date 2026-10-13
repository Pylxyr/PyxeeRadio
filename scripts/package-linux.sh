#!/usr/bin/env bash
# Assembles the Linux download: a folder with the app, the bot core and the bundled tools, as a .tar.gz.
#
#   scripts/package-linux.sh <version>
#
# Needs the pieces already built: app/src-tauri/target/release/PyxeeRadio (tauri build --no-bundle),
# dist/core/PyxeeRadioCore/ (PyInstaller) and packaging/bin/ (ffmpeg and qjs).
# Output: dist/app/PyxeeRadio-<version>-linux-x64.tar.gz
#
# The archive needs no installer and carries no web engine: it uses the system's WebKitGTK
# (webkit2gtk-4.1) and, for the tray icon, libayatana-appindicator.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

version="${1:?usage: scripts/package-linux.sh <version>}"
name="PyxeeRadio-${version}-linux-x64"
binary="app/src-tauri/target/release/PyxeeRadio"

for need in "$binary" dist/core/PyxeeRadioCore/PyxeeRadioCore packaging/bin/ffmpeg packaging/bin/qjs; do
  test -e "$need" || { echo "Missing: $need (build it first, see BUILD.md)" >&2; exit 1; }
done

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
root="$stage/$name"
mkdir -p "$root" dist/app

install -m 755 "$binary" "$root/PyxeeRadio"
cp -r dist/core/PyxeeRadioCore "$root/core"
mkdir -p "$root/bin"
cp -r packaging/bin/. "$root/bin/"
cp THIRD_PARTY_NOTICES.txt LICENSE "$root/"
install -m 644 app/src-tauri/icons/128x128.png "$root/PyxeeRadio.png"

cat > "$root/PyxeeRadio.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=PyxeeRadio
Comment=Twitch song-request radio
Exec=PyxeeRadio
Icon=PyxeeRadio
Categories=AudioVideo;Audio;
Terminal=false
DESKTOP

cat > "$root/README.txt" <<README
PyxeeRadio ${version}

Run ./PyxeeRadio from this folder. Nothing needs installing, but the system must provide:
  * WebKitGTK 4.1   (Arch: webkit2gtk-4.1   Debian/Ubuntu: libwebkit2gtk-4.1-0)
  * for the tray icon: libayatana-appindicator   (Arch: libayatana-appindicator   Debian/Ubuntu: libayatana-appindicator3-1)

Settings and data live in ~/.config/TwitchRadio. Delete this folder to remove the app.
To add it to your application menu, copy PyxeeRadio.desktop to ~/.local/share/applications/
and set its Exec= and Icon= lines to the full paths of PyxeeRadio and PyxeeRadio.png here.
README

tar -C "$stage" --owner=0 --group=0 -czf "dist/app/${name}.tar.gz" "$name"
ls -la "dist/app/${name}.tar.gz"
