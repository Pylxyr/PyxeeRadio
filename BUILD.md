# Building the PyxeeRadio desktop app (Windows and Linux)

The installer is made in five automatic steps: Python environment, ffmpeg and QuickJS, the bot core
(PyInstaller), the page checks, and the desktop app (Tauri: a Rust shell around a Svelte page).
This page is written for Windows; [Linux](#linux-arch--cachyos) has its own section below.

The desktop app uses the web engine the system already has (WebView2 on Windows, which Windows 10 and 11
include, and WebKitGTK on Linux) instead of shipping its own copy of Chromium, so it installs small and
idles with little memory. The page lives in `app/src` (Svelte + TypeScript), the shell in `app/src-tauri`
(Rust).

## Prerequisites (build machine only)

| Tool | Version | Get it |
|---|---|---|
| Python | 3.11 or newer, 64-bit, "Add python.exe to PATH" ticked | https://www.python.org/downloads/ |
| Node.js | 22.12 or newer (includes npm) | https://nodejs.org |
| Rust | stable, 1.90 or newer, with the Visual Studio C++ Build Tools | https://rustup.rs |
| Internet | for pip, npm, cargo, ffmpeg and QuickJS downloads | |

Users of the finished installer need **nothing** installed.

## Build

1. Open Command Prompt in the project folder.
2. Run:

   ```
   build-windows.bat
   ```

3. Wait (the first build takes several minutes and downloads a few hundred MB).
4. The installer appears at `app\src-tauri\target\release\bundle\nsis\PyxeeRadio_<version>_x64-setup.exe`.
   (The release workflow publishes it as `PyxeeRadio Setup <version>.exe`.) The program itself, without an
   installer, is `app\src-tauri\target\release\PyxeeRadio.exe`; it needs `core\` and `bin\` next to it.

If a step fails the script stops and says which one. Re-running resumes quickly: the Python
environment, downloaded tools (`packaging\.cache`), `app\node_modules` and the compiled Rust crates
(`app\src-tauri\target`) are reused. The first Rust build compiles a few hundred crates and takes several minutes.

## Try it before building

```
setup.bat          (once - creates .venv and installs requirements)
run-gui.bat        (starts the desktop app against your source checkout)
```

In this mode the app uses your existing `.env` and `data\` folder in the project directory.
Source installs need a JavaScript runtime for YouTube (the installed app brings its own, see
"YouTube JS solver" below): put `deno` (2.3 or newer) or `qjs` (QuickJS-ng, `python scripts/fetch_qjs.py
windows` puts one in `packaging\bin`) on PATH, or set `YTDLP_JS_RUNTIME_PATH` in `.env`.

## What ends up where (installed app)

| Item | Location |
|---|---|
| Program | `%LOCALAPPDATA%\Programs\PyxeeRadio\` |
| Your settings, tokens, queue | `%APPDATA%\TwitchRadio\` (`.env`, `data\`, `logs\`) - kept on uninstall unless you tick the box the uninstaller offers (default: keep) |
| App window preferences | `%APPDATA%\TwitchRadio\gui\` |
| Updated yt-dlp (if you installed one) | `%APPDATA%\TwitchRadio\yt-dlp\` (`current\`, `previous\`, `state.json`) |
| Downloaded JS solver scripts | `%APPDATA%\TwitchRadio\data\yt-dlp-cache\challenge-solver\` |

To move an existing install: copy your old `.env` and `data\` folder into `%APPDATA%\TwitchRadio\`.

## Pinned tools

### ffmpeg

The installer and tarball do not ship a downloaded ffmpeg. They ship a small one (under 10 MB, against
80 to 105 MB for the general-purpose builds) that `packaging/ffmpeg/build.sh` compiles from the FFmpeg and
libopus source tarballs pinned, with SHA-256, in `packaging/tools.lock.json`. It contains only what the app
uses (decode what YouTube serves, encode Opus in Ogg; songs are not otherwise processed), and it is LGPL,
not GPL. CI builds it for both platforms on every release (`.github/workflows/ffmpeg.yml`) and on any pull
request that touches `packaging/`, and runs `scripts/check_ffmpeg.py` on the result: that script is the
list of what the app needs from ffmpeg, and it runs the app's own command lines against every container
YouTube serves (WebM/Opus, M4A/AAC, HLS with and without video, encrypted HLS) over local HTTP and HTTPS.

* **Release / CI:** nothing to do; the jobs build, check and package it.
* **Linux, locally:** `scripts/build-linux.sh` builds it into `packaging/bin` (needs a C toolchain, `nasm`,
  `cmake` and OpenSSL development files; the first build takes a few minutes).
* **Windows, locally:** the Windows ffmpeg is cross-compiled with mingw-w64, so `build-windows.bat` needs
  `packaging\bin\ffmpeg.exe` to exist and never bundles one found on PATH. Either download the
  `ffmpeg-windows-x64` artifact from a CI run, or run `bash packaging/ffmpeg/build.sh windows packaging/bin`
  in WSL (`sudo apt install build-essential nasm cmake pkg-config curl mingw-w64`).
* **Bumping ffmpeg or libopus:** change the version, URL and SHA-256 in `tools.lock.json` (the build refuses
  a file whose hash does not match), and let CI run. **Adding something the app needs** (a new codec,
  filter or protocol): add it to `build.sh` and to `REQUIRED` in `scripts/check_ffmpeg.py` together.
* **Run the player tests against the shipped build:**
  `FIXTURE_FFMPEG=$(which ffmpeg) PATH="$PWD/packaging/bin:$PATH" pytest tests/test_player_e2e.py`.

`THIRD_PARTY_NOTICES.txt` (repo root) is copied into the installer. Keep it current when you add
a dependency or change a tool.

## Checks you can run locally

```
pip install -r requirements-dev.txt -r requirements.txt
ruff check .
mypy twitch_radio bot.py
pytest -q

cd app
npm ci
npm run check          # svelte-check: types and Svelte warnings
npm test               # vitest: logic, theme contrast of every colour style, and the components mounted in jsdom
npm run build          # the production page (app/dist), which the shell compiles into itself

cd src-tauri
cargo test             # the shell: validation, preferences, update check, log formatting
cargo clippy --all-targets -- -D warnings
```

On Linux the shell links against the system WebKitGTK; install its development files first (the list is
in the Linux section below). `npx tauri dev` (or `run-gui.bat`) starts the app against your source checkout
with hot reload for the page.

After a core build, smoke-test the frozen exe (catches hidden-import and data-file problems that
only appear when frozen):

```
python scripts\smoke_core.py --exe dist\core\PyxeeRadioCore\PyxeeRadioCore.exe --bin packaging\bin
python scripts\smoke_core.py --exe dist\core\PyxeeRadioCore\PyxeeRadioCore.exe --bin packaging\bin --resolve-only
```

The second command resolves a real YouTube video and needs YouTube and GitHub reachable.

`ruff format --check .` runs in CI; fix a failure with `ruff format .`.

## Linux (Arch / CachyOS)

Releases include `PyxeeRadio-x.y.z-linux-x64.tar.gz` (x86-64): the app, the bot core, ffmpeg and QuickJS in
one folder. There is no installer and no bundled web engine; the app uses the system's WebKitGTK.

**Run a release build**

```
sudo pacman -S webkit2gtk-4.1 libayatana-appindicator     # once; Debian/Ubuntu: libwebkit2gtk-4.1-0 libayatana-appindicator3-1
tar xzf PyxeeRadio-*-linux-x64.tar.gz
./PyxeeRadio-*-linux-x64/PyxeeRadio
```

Settings, tokens and data live in `~/.config/TwitchRadio` (the Windows `%APPDATA%\TwitchRadio` equivalent).
Delete the folder to remove the app. The tray icon needs `libayatana-appindicator` and a desktop that shows
status icons (KDE, or GNOME with the AppIndicator extension); without one the app still runs, you just
cannot reach it again after closing the window to the tray, so turn off "hide to tray" there.

**Why not an AppImage?** An AppImage has to carry WebKitGTK itself (about 70 MB more), which is the weight
this app was moved off Electron to avoid. `npx tauri build --bundles appimage` still makes one if you want it.

**Build it yourself**

```
sudo pacman -S python nodejs npm rustup curl base-devel nasm cmake openssl webkit2gtk-4.1 libayatana-appindicator librsvg
rustup default stable
scripts/build-linux.sh
```

This runs the same five steps as `build-windows.bat` and leaves the tarball in `dist/app/`. Step 2 builds the
small ffmpeg described above from pinned source (and skips it when `packaging/bin/ffmpeg` already exists), and
downloads the pinned QuickJS. ffmpeg links OpenSSL and libopus statically and needs only glibc, libm and the
kernel, so it runs wherever the bot's frozen core does.

"Open PyxeeRadio when I sign in" works on Linux too (it writes an autostart entry). Install and update from the
app only shows a link to the download; nothing is installed for you. Only x86-64 is built.

**Source install on Linux** (no packaging): `sudo pacman -S python nodejs npm ffmpeg deno rustup webkit2gtk-4.1`,
then `python -m venv .venv && .venv/bin/pip install -r requirements.txt` and, for the desktop shell,
`cd app && npm ci && npx tauri dev`. The bot uses `ffmpeg` and `deno` from PATH.

## Cutting a release

On GitHub: **Actions > Cut release > Run workflow**, pick `patch`, `minor` or `major`, and run it.
(To set an exact version, such as `1.2.0` or `1.2.0-rc1`, type it into the version box instead.)
The workflow then, with no further steps from you:

1. works out the next version from the current one (the very first release uses the version already
   in the files, so a repo at 1.0.0 with no tags releases 1.0.0),
2. commits the bump to `main` and tags it (`bump-version.py` sets `app/package.json`, `app/package-lock.json`,
   `app/src-tauri/Cargo.toml`, `Cargo.lock`, `tauri.conf.json` and `twitch_radio/version.py`),
3. builds the Windows installer and the Linux tarball, and publishes the GitHub release
   by calling **release.yml**.

A version with a `-` suffix (`1.2.0-rc1`) is published as a pre-release. If the build fails after the
tag was created, fix the problem and run **Cut release** again: the failed version number is simply
skipped. If the failure was a flaky network step, open the failed run and choose **Re-run failed jobs**.

Prefer a terminal? The manual route still works and gives the same result:

```
python scripts\bump-version.py 1.1.0
git add -A
git commit -m "Release 1.1.0"
git tag v1.1.0
git push origin main v1.1.0
```

For a dry run, start the **Release** workflow by hand (Actions > Release > Run workflow) with the
version: it builds everything and uploads the files as a workflow artifact without publishing.

## How CI works

* **ci.yml** runs on pushes and pull requests to `main`; a newer push cancels the older run.
  * `python`: Python 3.11 and 3.12 - `ruff check`, `mypy`, `pytest`.
  * `app`: the page (`npm ci`, type check, tests, build) and the shell (`cargo test`, `cargo clippy -D warnings`).
  * `windows-build-smoke`: only on pull requests that touch `twitch_radio/`, `packaging/`, `app/` or
    `requirements*`. Builds the core with PyInstaller and runs `scripts/smoke_core.py`. The last
    step (real YouTube resolve, needs the network) may fail without failing the PR.
* **cut-release.yml** is the one-click release button (see "Cutting a release"). It bumps, commits
  and tags on `main`, then calls release.yml, because a tag pushed with the built-in token does not
  start other workflows by itself.
* **release.yml** runs on `v*.*.*` tags, and when called by cut-release.yml. It first checks that the tag equals the version in
  `app/package.json`, `app/package-lock.json`, the Rust crate and `tauri.conf.json` and `twitch_radio/version.py`, then builds on
  Windows (pinned tools, PyInstaller core, smoke tests, then the NSIS installer, which it installs silently and
  smoke-tests from where it landed; the real YouTube
  resolve is best effort here because YouTube often blocks GitHub's runners, so a failure is a
  warning in the run, not a stopped release), writes `SHA256SUMS.txt`, and publishes a GitHub
  release with the installer, `SHA256SUMS.txt` and `THIRD_PARTY_NOTICES.txt`.
  Release notes are a changelog built by `scripts/release_notes.py` from the changes since the previous release: commit messages grouped into Added / Fixed / Changed /
  Removed / Dependencies (release-bump, `ruff format` and merge commits are left out). For vague
  messages such as "Add files via upload" or "Update bot.py" it reads the code instead and lists new
  or removed chat commands, settings and functions plus the files changed per area. With the optional
  `ANTHROPIC_API_KEY` repository secret, Claude reads the diff and writes the notes in plain language
  (any error falls back to the above). A compare link is added, under the fixed install note from
  `.github/release-notes-header.md`. A stable release is compared with the previous stable release,
  not with a release candidate.
* Third-party actions are pinned to full commit SHAs with the version in a comment.
  `.github/dependabot.yml` opens weekly pull requests for GitHub Actions, pip, npm and cargo, which is how
  they stay fresh. Review those PRs (Dependabot updates the SHA and the comment together).

Code signing is optional: set the repository secrets `WIN_CSC_LINK` (a base64 `.pfx`, or a URL) and
`WIN_CSC_KEY_PASSWORD`. The workflow imports the certificate into the runner's store and Tauri's bundler signs
the app and the installer by thumbprint (`scripts/tauri_signing_config.py` writes that config). Without the
secrets the installer is built unsigned and the release notes say so.
Build provenance (`actions/attest-build-provenance`) needs a public repository, or GitHub Enterprise
Cloud for a private one; where it isn't available the step is skipped without failing the release.

## How the update mechanisms behave

**YouTube JS solver.** yt-dlp needs a JavaScript runtime and "solver" scripts to read some YouTube
links. The installed app ships **QuickJS-ng** (`bin\qjs.exe`, about 2 MB) as that runtime, and the core
selects it automatically in the packaged app (`twitch_radio/config.py`). It is downloaded by
`scripts/fetch_qjs.py` from the pinned release in `packaging/tools.lock.json` and checked against its SHA-256.
Deno would be 93 MB, and the Electron-era trick of running the app's own executable as Node is gone along
with Electron. QuickJS is slower than Node or Deno at solving the player's challenge (seconds, not
milliseconds), and the core already tries a quick attempt first and falls back when it is too slow. A Deno
placed in `bin\` or set via `YTDLP_JS_RUNTIME_PATH` still takes precedence, and `YTDLP_JS_RUNTIME_NAME`
can select `node` or `bun` for a source install. The `yt-dlp-ejs` pip package is deliberately **not** bundled. yt-dlp is
started with `remote_components: ["ejs:github"]`, downloads the scripts that match its own version
from github.com/yt-dlp/ejs over HTTPS the first time it needs them, and caches them in
`data\yt-dlp-cache`, so they survive restarts. If the download is blocked, the Logs tab shows an
error after start-up, the setup checklist keeps "Download the YouTube JS solver" open, and
Settings > App shows "JS solver: not downloaded yet". Allow github.com and restart the bot.
yt-dlp runs the scripts with the runtime's permission model on (`--permission`, no grants), so they
get no file, network, environment or process access, and it checks the downloaded script against a
hash it ships with.

**yt-dlp updates.** yt-dlp is frozen into the core, but Settings > App can install a newer release
without a new installer. The core downloads the release's `yt-dlp` zipimport file and
`SHA2-256SUMS` from github.com (HTTPS only, every redirect checked), verifies the SHA-256 before
unpacking anything, unpacks only the `yt_dlp/` package into `%APPDATA%\TwitchRadio\yt-dlp\staging`,
imports it once in a child process, then swaps it in as `current` and keeps the old one as
`previous` ("Roll back" in Settings). At start-up the core and every extractor worker put
`current` ahead of the bundled copy on `sys.path`, but only if it is newer than the bundled one;
if it fails to import, the bundled copy is used and the failure is remembered in `state.json`.
Updating restarts the bot. With "Check for yt-dlp updates on startup" on (default), the app only
checks, never installs on its own. Everything is logged in the Logs tab.
Limit: the checksum file comes from the same release, so it detects corruption and tampering in
transit, not a compromised release (yt-dlp's GPG signature is not checked).

**App updates.** Settings > App > "Check for app updates" asks the GitHub Releases API for the
latest release of `Pylxyr/Twitch-Radio` (constant `UPDATE_REPO` in `app/src-tauri/src/updates.rs`), compares it
with the running version and shows a download link, opened through the app's safe external-link
handler. Nothing is installed automatically. Automatic checks run at most once a day and can be
turned off in the same card.

## Things to check on first build

* **Antivirus / SmartScreen.** An unsigned installer shows "Windows protected your PC" and
  PyInstaller executables are sometimes flagged by antivirus. Both are normal for unsigned apps; a
  code-signing certificate fixes SmartScreen. Add the install folder to your antivirus exclusions if
  it quarantines `PyxeeRadioCore.exe`.
* **ffmpeg licence.** The shipped ffmpeg is an LGPL build (v2.1 on Windows, v3 on Linux), with no GPL
  or non-free parts. If you distribute the installer, keep `bin\FFMPEG-LICENSE.txt` and
  `THIRD_PARTY_NOTICES.txt` in it, and keep `packaging/ffmpeg/build.sh` and the lock file public: they
  are the source offer. See https://ffmpeg.org/legal.html.
