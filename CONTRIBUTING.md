# Contributing

## Development

```
pip install -r requirements-dev.txt -r requirements.txt
ruff check .
ruff format --check .
mypy twitch_radio bot.py          # disallow_untyped_defs is on
pytest -q

cd app
npm ci
npm run check            # svelte-check: types and Svelte warnings
npm test                 # vitest
cd src-tauri
cargo test
cargo clippy --all-targets -- -D warnings
```

The Rust shell needs the WebKitGTK development files on Linux (list in [BUILD.md](BUILD.md)) and the
page built once (`cd app && npm run build`), because the shell compiles the built page into itself.

CI runs the same commands. Please also run them before opening a pull request.

## Ground rules

* The architecture stays: the Rust shell (`app/src-tauri`) spawns the Python core as a hidden child and
  they speak JSON lines on stdin/stdout (protocol at the top of `twitch_radio/headless.py`). The page
  (`app/src`, Svelte) only talks to the shell through the commands in `commands.rs`.
* The page's security stays strict: a CSP with no network access (`connect-src` is the shell's IPC only),
  no `{@html}` of anything that is not a fixed string, and a capability file
  (`capabilities/default.json`) that lists the few window actions it may take. Files, dialogs,
  processes, links and all network access happen in the shell or in the core, never in the page, and
  the shell re-validates everything the page sends.
* Pins are exact in `requirements*.txt`, `app/package.json` and `app/src-tauri/Cargo.lock` (commit the lock file; CI builds with `--locked`). When you bump yt-dlp, re-read its
  `pin` extra (`pip show yt-dlp`, its `pyproject.toml`) and update the companion pins in
  `requirements.txt` to match. Do not add the `yt-dlp-ejs` package, or yt-dlp's optional extras
  (`curl-cffi`, `mutagen`, `pycryptodomex`, ...): the app doesn't use them and they roughly double
  the frozen core.
* Release builds install from `requirements-build.lock` with `--require-hashes`. After changing
  `requirements.txt` or `requirements-build.txt`, regenerate it with the command in its header; the
  `build-lock` CI job fails if it is out of date.
* The installed app has no Node: yt-dlp's JavaScript runtime is the bundled QuickJS
  (`scripts/fetch_qjs.py`, pinned in `packaging/tools.lock.json`). Do not add a bigger runtime to the installer.
* Skin names live in `app/shared/skin-names.json`, read by both the page and the shell; the limits for the
  radio lookahead are checked against `twitch_radio/lookahead.py` by a test.
* Version numbers: use `python scripts/bump-version.py X.Y.Z`; never edit the places by hand.
* Comments: only for a non-obvious "why".
