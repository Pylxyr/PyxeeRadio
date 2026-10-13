"""scripts/fetch_qjs.py keeps a download only when its hash matches the pinned one."""

from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path

import pytest

_SPEC = importlib.util.spec_from_file_location(
    "fetch_qjs", Path(__file__).resolve().parent.parent / "scripts" / "fetch_qjs.py"
)
assert _SPEC and _SPEC.loader
fetch_qjs = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(fetch_qjs)


def test_the_pinned_builds_are_well_formed() -> None:
    for platform, name in (("linux", "qjs"), ("windows", "qjs.exe")):
        entry = fetch_qjs.load_entry(platform)
        assert entry["file"] == name
        assert entry["url"].startswith("https://github.com/quickjs-ng/quickjs/releases/download/")
        assert len(entry["sha256"]) == 64 and int(entry["sha256"], 16) >= 0


def test_unknown_platform_is_refused(tmp_path: Path) -> None:
    lock = tmp_path / "lock.json"
    lock.write_text(json.dumps({"quickjs": {"assets": {}}}))
    with pytest.raises(SystemExit):
        fetch_qjs.load_entry("linux", lock)


def test_a_matching_download_is_kept_and_a_wrong_one_is_not(tmp_path: Path) -> None:
    source = tmp_path / "qjs-src"
    source.write_bytes(b"pretend runtime")
    good = hashlib.sha256(b"pretend runtime").hexdigest()
    target = tmp_path / "out" / "qjs"
    fetch_qjs.download(source.as_uri(), good, target)
    assert target.read_bytes() == b"pretend runtime"
    assert not list(target.parent.glob("*.part"))

    other = tmp_path / "out2" / "qjs"
    with pytest.raises(SystemExit, match="SHA-256 mismatch"):
        fetch_qjs.download(source.as_uri(), "0" * 64, other)
    assert not other.exists() and not list(other.parent.glob("*.part"))


def test_only_github_or_a_local_file_is_downloaded_from(tmp_path: Path) -> None:
    with pytest.raises(SystemExit, match="Refusing"):
        fetch_qjs.download("https://evil.example/qjs", "0" * 64, tmp_path / "qjs")
    with pytest.raises(SystemExit, match="Refusing"):
        fetch_qjs.download("http://github.com/x/qjs", "0" * 64, tmp_path / "qjs")
