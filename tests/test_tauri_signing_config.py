import importlib.util
import json
from pathlib import Path

import pytest

_SPEC = importlib.util.spec_from_file_location(
    "tauri_signing_config", Path(__file__).resolve().parent.parent / "scripts" / "tauri_signing_config.py"
)
assert _SPEC and _SPEC.loader
tauri_signing_config = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(tauri_signing_config)

THUMB = "ab12" * 10


def test_the_fragment_names_the_certificate_and_a_timestamp_server() -> None:
    windows = tauri_signing_config.signing_config(THUMB)["bundle"]["windows"]  # type: ignore[index]
    assert windows["certificateThumbprint"] == THUMB.upper()
    assert windows["digestAlgorithm"] == "sha256"
    assert windows["timestampUrl"].startswith("http")


def test_spaces_are_dropped_and_anything_else_is_refused() -> None:
    spaced = " ".join(THUMB[i : i + 4] for i in range(0, 40, 4))
    assert (
        tauri_signing_config.signing_config(spaced)["bundle"]["windows"]["certificateThumbprint"]
        == THUMB.upper()
    )  # type: ignore[index]
    for bad in ("", "abc", "g" * 40, THUMB + "0"):
        with pytest.raises(ValueError):
            tauri_signing_config.signing_config(bad)


def test_the_file_is_written(tmp_path: Path) -> None:
    out = tmp_path / "sign.json"
    assert tauri_signing_config.main(["x", THUMB, str(out)]) == 0
    assert json.loads(out.read_text())["bundle"]["windows"]["certificateThumbprint"] == THUMB.upper()
    assert tauri_signing_config.main(["x", "nope", str(out)]) == 1
    assert tauri_signing_config.main(["x"]) == 2
