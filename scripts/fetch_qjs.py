"""Downloads the QuickJS runtime that ships with the installed app into packaging/bin.

yt-dlp needs a JavaScript runtime to read some YouTube links. The installed app brings QuickJS-ng
(about 2 MB) instead of Deno (93 MB). The release file is pinned by URL and SHA-256 in
packaging/tools.lock.json, and nothing is kept unless the hash matches.

    python scripts/fetch_qjs.py linux   [--out packaging/bin]
    python scripts/fetch_qjs.py windows [--out packaging/bin]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOCK = ROOT / "packaging" / "tools.lock.json"
MAX_BYTES = 16 * 1024 * 1024  # a runtime this size is wrong: stop reading rather than fill the disk


def load_entry(platform: str, lock: Path = LOCK) -> dict[str, str]:
    entry = json.loads(lock.read_text(encoding="utf-8"))["quickjs"]["assets"].get(platform)
    if not entry:
        raise SystemExit(f"No QuickJS build is pinned for {platform!r} in {lock.name}.")
    return entry


def download(url: str, expected_sha256: str, target: Path) -> None:
    """Fetches `url` to `target`, but only keeps it if its SHA-256 is `expected_sha256`."""
    if not url.startswith(("https://github.com/", "file:")):
        raise SystemExit(f"Refusing to download from {url!r}: only github.com is expected.")
    digest = hashlib.sha256()
    tmp = target.with_name(target.name + ".part")
    target.parent.mkdir(parents=True, exist_ok=True)
    try:
        with urllib.request.urlopen(url, timeout=60) as response, tmp.open("wb") as out:  # noqa: S310 - scheme checked above
            size = 0
            while chunk := response.read(65536):
                size += len(chunk)
                if size > MAX_BYTES:
                    raise SystemExit("The download is far larger than QuickJS; refusing it.")
                digest.update(chunk)
                out.write(chunk)
        if digest.hexdigest() != expected_sha256.lower():
            raise SystemExit(
                f"SHA-256 mismatch for {url}:\n  expected {expected_sha256}\n  got      {digest.hexdigest()}"
            )
        os.replace(tmp, target)
    finally:
        tmp.unlink(missing_ok=True)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("platform", choices=["linux", "windows"])
    parser.add_argument("--out", default=str(ROOT / "packaging" / "bin"), help="folder to put qjs in")
    args = parser.parse_args(argv)
    entry = load_entry(args.platform)
    target = Path(args.out) / entry["file"]
    download(entry["url"], entry["sha256"], target)
    if args.platform == "linux":
        target.chmod(0o755)
    print(f"[ok] {target} ({target.stat().st_size:,} bytes, sha256 verified)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
