#!/usr/bin/env python3
"""Smoke test for a PyInstaller-built PyxeeRadioCore.

Catches what only breaks when frozen (missing hidden imports, data files,
bundled-vs-installed yt-dlp). Uses a throwaway TWITCH_RADIO_HOME.

  python scripts/smoke_core.py --exe dist/core/PyxeeRadioCore/PyxeeRadioCore.exe
  python scripts/smoke_core.py --exe ... --resolve     # needs YouTube and GitHub reachable

Exit code 0 only if every selected check passes.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

DEFAULT_URL = "https://www.youtube.com/watch?v=jNQXAC9IVRw"  # "Me at the zoo"
failures: list[str] = []


def check(ok: bool, label: str, detail: str = "") -> None:
    print(f"[{'ok' if ok else 'FAIL'}] {label}{' - ' + detail if detail and not ok else ''}")
    if not ok:
        failures.append(label)


def run(
    exe: str, args: list[str], env: dict[str, str], stdin: str = "", timeout: int = 120
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [exe, *args], input=stdin, capture_output=True, text=True, env=env, timeout=timeout, check=False
    )


def last_json(text: str) -> dict[str, object]:
    for line in reversed(text.strip().splitlines()):
        try:
            value = json.loads(line)
        except ValueError:
            continue
        if isinstance(value, dict):
            return value
    return {}


def basic(exe: str, env: dict[str, str], bin_dir: str | None) -> None:
    done = run(exe, ["--preflight"], env)
    report = last_json(done.stdout)
    check(bool(report), "--preflight prints JSON", done.stderr[-300:])
    check(report.get("frozen") is True, "preflight sees a frozen build")
    check(
        isinstance(report.get("ytdlp"), dict) and report["ytdlp"]["source"] == "bundled",
        "bundled yt-dlp is in use",
        str(report.get("ytdlp")),
    )  # type: ignore[index]
    check(isinstance(report.get("js_solver"), dict), "preflight reports the JS solver")
    if bin_dir:
        ffmpeg = str((report.get("ffmpeg") or {}).get("path") or "")  # type: ignore[attr-defined]
        check(
            ffmpeg.startswith(str(Path(bin_dir).resolve()))
            and bool((report.get("ffmpeg") or {}).get("version")),  # type: ignore[attr-defined]
            "bundled ffmpeg is found and runs",
            str(report.get("ffmpeg")),
        )
        stale = [p.name for p in Path(bin_dir).glob("deno*")]
        check(not stale, "no Deno is bundled (it alone is 93 MB; QuickJS is the runtime)", str(stale))
        # The QuickJS that ships with the app is yt-dlp's JavaScript runtime in the frozen core.
        runtime = report.get("js_runtime") or {}
        got = str(runtime.get("path") or "")  # type: ignore[attr-defined]
        check(
            runtime.get("name") == "quickjs"  # type: ignore[attr-defined]
            and bool(got)
            and Path(got).resolve().parent == Path(bin_dir).resolve(),
            "frozen core uses the bundled QuickJS as its JavaScript runtime",
            str(runtime),
        )

    done = run(exe, ["--env-json"], env)
    check(
        done.returncode == 0 and isinstance(last_json(done.stdout), dict),
        "--env-json reads the .env",
        done.stderr[-300:],
    )

    done = run(exe, ["--ytdlp-selftest"], env)
    report = last_json(done.stdout)
    check(report.get("ok") is True, "yt_dlp imports in the frozen core", str(report))
    check(report.get("yt_dlp_ejs_importable") is False, "yt_dlp_ejs is NOT bundled", str(report))

    worker = subprocess.Popen(
        [exe, "--extractor-worker"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env,
    )
    try:
        assert worker.stdout is not None
        banner = last_json(worker.stdout.readline())
        check(banner.get("ready") is True, "--extractor-worker starts and says ready", str(banner))
    finally:
        if worker.stdin:
            worker.stdin.close()
        try:
            worker.wait(timeout=20)
        except subprocess.TimeoutExpired:
            worker.kill()


def headless(exe: str, env: dict[str, str]) -> None:
    """The channel the desktop app's Logs tab and command answers depend on: `--headless` speaks JSON
    lines on stdout. Stdin is closed at once, which makes the core stop itself (the app is "gone"),
    so this needs no network and ends in well under a second."""
    done = run(exe, ["--headless"], env, stdin="", timeout=90)
    lines = [line for line in done.stdout.splitlines() if line.strip()]
    messages: list[dict[str, object]] = []
    for line in lines:
        try:
            value = json.loads(line)
        except ValueError:
            value = None
        messages.append(value if isinstance(value, dict) else {})
    kinds = [str(m.get("t")) for m in messages]
    check(
        done.returncode == 0, "--headless stops cleanly when the app closes stdin", f"exit {done.returncode}"
    )
    check(
        bool(lines) and all(messages), "every stdout line of --headless is a JSON object", done.stdout[-300:]
    )
    check(kinds[:1] == ["hello"], "--headless starts with a hello message", str(kinds[:3]))
    check("log" in kinds, "--headless sends log messages (what the Logs tab shows)", str(kinds))
    check("phase" in kinds, "--headless reports its phase", str(kinds))


def resolve(exe: str, env: dict[str, str], home: Path, url: str) -> None:
    done = run(exe, ["--resolve-test", url], {**env, "YTDLP_EXTRACT_TIMEOUT_SECONDS": "120"}, timeout=400)
    report = last_json(done.stdout)
    check(
        report.get("ok") is True,
        "real YouTube resolve with the frozen core",
        f"{report.get('error')} {done.stderr[-400:]}",
    )
    cached = home / "data" / "yt-dlp-cache" / "challenge-solver" / "lib.json"
    check(
        cached.is_file() and cached.stat().st_size > 1000,
        "JS solver script downloaded and cached under data/",
        str(cached),
    )


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--exe", required=True)
    parser.add_argument("--resolve", action="store_true", help="also do the real YouTube resolve")
    parser.add_argument(
        "--resolve-only",
        action="store_true",
        help="skip the basic checks (so CI can treat the network step separately)",
    )
    parser.add_argument("--url", default=DEFAULT_URL)
    parser.add_argument("--home", help="data folder to use (default: a temp folder)")
    parser.add_argument(
        "--bin",
        help="folder with the bundled ffmpeg and qjs (packaging/bin); the desktop app passes it as TWITCH_RADIO_BIN",
    )
    args = parser.parse_args()
    exe = str(Path(args.exe).resolve())
    home = Path(args.home or tempfile.mkdtemp(prefix="twitch-radio-smoke-")).resolve()
    home.mkdir(parents=True, exist_ok=True)
    env = {
        **os.environ,
        "TWITCH_RADIO_HOME": str(home),
        "TWITCH_CLIENT_ID": "smoke",
        "TWITCH_CLIENT_SECRET": "smoke",
        "TWITCH_BOT_ID": "1",
        "TWITCH_OWNER_ID": "2",
    }
    if args.bin:
        env["TWITCH_RADIO_BIN"] = str(Path(args.bin).resolve())
    for name in ("YTDLP_PLAYER_CLIENT", "YTDLP_JS_RUNTIME_PATH", "TWITCH_RADIO_NO_YTDLP_OVERRIDE"):
        env.pop(name, None)
    print(f"exe: {exe}\nhome: {home}")
    if not args.resolve_only:
        basic(exe, env, args.bin)
        headless(exe, env)
    if args.resolve or args.resolve_only:
        resolve(exe, env, home, args.url)
    print("\nFAILED: " + "; ".join(failures) if failures else "\nall checks passed")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
