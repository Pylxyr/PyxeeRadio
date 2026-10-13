#!/usr/bin/env python3
"""Keeps the app version in one step across the places that carry it.

python scripts/bump-version.py 1.1.0          set app/package.json, app/package-lock.json,
                                              app/src-tauri/Cargo.toml, Cargo.lock, tauri.conf.json
                                              and twitch_radio/version.py, print next steps
python scripts/bump-version.py --check v1.1.0 fail unless all of them equal the tag (used by CI)
python scripts/bump-version.py --next patch   print the next version (patch, minor or major) without
                                              changing anything (used by the Cut release workflow)
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
APP = ROOT / "app"
PACKAGE = APP / "package.json"
LOCK = APP / "package-lock.json"
CARGO_TOML = APP / "src-tauri" / "Cargo.toml"
CARGO_LOCK = APP / "src-tauri" / "Cargo.lock"
TAURI_CONF = APP / "src-tauri" / "tauri.conf.json"
VERSION_PY = ROOT / "twitch_radio" / "version.py"
SEMVER = re.compile(r"^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$")
VERSION_LINE = re.compile(r'^APP_VERSION = "([^"]*)"', re.MULTILINE)
# The first `version = "..."` of the [package] table, and the one right after `name = "pyxee-radio"` in the lock file.
CARGO_TOML_VERSION = re.compile(r'(\[package\][^\[]*?^version = ")([^"]*)(")', re.MULTILINE | re.DOTALL)
CARGO_LOCK_VERSION = re.compile(r'(^name = "pyxee-radio"\nversion = ")([^"]*)(")', re.MULTILINE)


def _regex_version(path: Path, pattern: re.Pattern[str]) -> str | None:
    if not path.exists():
        return None  # a checkout that never built the desktop app has no lock file yet
    match = pattern.search(path.read_text(encoding="utf-8"))
    return match.group(2) if match else None


def _write_regex_version(path: Path, pattern: re.Pattern[str], version: str) -> None:
    if not path.exists():
        return
    text = path.read_text(encoding="utf-8")
    path.write_text(
        pattern.sub(lambda m: f"{m.group(1)}{version}{m.group(3)}", text, count=1), encoding="utf-8"
    )


def read_versions() -> dict[str, str | None]:
    match = VERSION_LINE.search(VERSION_PY.read_text(encoding="utf-8"))
    lock = json.loads(LOCK.read_text(encoding="utf-8")) if LOCK.exists() else {}
    return {
        "app/package.json": json.loads(PACKAGE.read_text(encoding="utf-8")).get("version"),
        "app/package-lock.json": lock.get("version"),
        "app/package-lock.json (root package)": (lock.get("packages", {}).get("") or {}).get("version"),
        "app/src-tauri/Cargo.toml": _regex_version(CARGO_TOML, CARGO_TOML_VERSION),
        "app/src-tauri/Cargo.lock": _regex_version(CARGO_LOCK, CARGO_LOCK_VERSION),
        "app/src-tauri/tauri.conf.json": json.loads(TAURI_CONF.read_text(encoding="utf-8")).get("version"),
        "twitch_radio/version.py": match.group(1) if match else None,
    }


def check(tag: str) -> int:
    expected = tag[1:] if tag.startswith("v") else tag
    if not SEMVER.match(expected):
        print(f"'{tag}' is not a vMAJOR.MINOR.PATCH[-suffix] tag.", file=sys.stderr)
        return 1
    bad = {name: found for name, found in read_versions().items() if found != expected}
    for name, found in read_versions().items():
        print(f"{'ok  ' if found == expected else 'FAIL'} {name}: {found}")
    if bad:
        print(
            f"\nThe tag says {expected} but the files above disagree. Run: python scripts/bump-version.py {expected}",
            file=sys.stderr,
        )
        return 1
    return 0


def next_version(current: str, kind: str) -> str:
    """The version after `current` for a patch, minor or major release.

    A pre-release suffix on `current` is dropped first, so 1.2.0-rc1 + patch is 1.2.1;
    to promote a release candidate to 1.2.0, give the exact version instead.
    """
    match = SEMVER.match(current)
    if not match:
        raise ValueError(f"'{current}' is not MAJOR.MINOR.PATCH[-suffix].")
    major, minor, patch = (int(part) for part in current.split("-", 1)[0].split("."))
    if kind == "major":
        return f"{major + 1}.0.0"
    if kind == "minor":
        return f"{major}.{minor + 1}.0"
    if kind == "patch":
        return f"{major}.{minor}.{patch + 1}"
    raise ValueError(f"Unknown bump kind '{kind}' (use patch, minor or major).")


def print_next(kind: str) -> int:
    current = read_versions()["app/package.json"]
    try:
        print(next_version(current or "", kind))
    except ValueError as exc:
        print(exc, file=sys.stderr)
        return 1
    return 0


def write_json_version(path: Path, version: str) -> None:
    data = json.loads(path.read_text(encoding="utf-8"))
    data["version"] = version
    if "packages" in data and "" in data["packages"]:
        data["packages"][""]["version"] = version
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def bump(version: str) -> int:
    if not SEMVER.match(version):
        print(f"'{version}' is not MAJOR.MINOR.PATCH (optionally -rc1 etc.).", file=sys.stderr)
        return 1
    write_json_version(PACKAGE, version)
    if LOCK.exists():
        write_json_version(LOCK, version)
    _write_regex_version(CARGO_TOML, CARGO_TOML_VERSION, version)
    _write_regex_version(CARGO_LOCK, CARGO_LOCK_VERSION, version)
    conf = json.loads(TAURI_CONF.read_text(encoding="utf-8"))
    conf["version"] = version
    TAURI_CONF.write_text(json.dumps(conf, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    text = VERSION_PY.read_text(encoding="utf-8")
    if not VERSION_LINE.search(text):
        print("APP_VERSION not found in twitch_radio/version.py", file=sys.stderr)
        return 1
    VERSION_PY.write_text(VERSION_LINE.sub(f'APP_VERSION = "{version}"', text, count=1), encoding="utf-8")
    print(
        f"Version set to {version} in app/package.json, app/package-lock.json, app/src-tauri/Cargo.toml, "
        f"Cargo.lock, tauri.conf.json and twitch_radio/version.py.\n"
    )
    print("Next steps:")
    print(f'  git add -A && git commit -m "Release {version}"')
    print(f"  git tag v{version}")
    print(f"  git push origin main v{version}")
    print(
        "The release workflow starts when the tag arrives; versions that contain '-' publish as pre-releases."
    )
    return 0


def main(argv: list[str]) -> int:
    if len(argv) == 3 and argv[1] == "--check":
        return check(argv[2])
    if len(argv) == 3 and argv[1] == "--next":
        return print_next(argv[2])
    if len(argv) == 2 and not argv[1].startswith("-"):
        return bump(argv[1].removeprefix("v"))
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
