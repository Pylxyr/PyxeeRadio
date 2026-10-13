"""Writes the Tauri config fragment that signs the Windows build with a certificate in the user's store.

    python scripts/tauri_signing_config.py <thumbprint> app/src-tauri/tauri.signing.conf.json

The release workflow imports the certificate from its secrets first, then passes this file to
`tauri build --config`, which merges it over the base config.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

TIMESTAMP_URL = "http://timestamp.digicert.com"


def signing_config(thumbprint: str) -> dict[str, object]:
    cleaned = thumbprint.replace(" ", "").upper()
    if not re.fullmatch(r"[0-9A-F]{40}", cleaned):
        raise ValueError("A certificate thumbprint is 40 hexadecimal characters.")
    return {
        "bundle": {
            "windows": {
                "certificateThumbprint": cleaned,
                "digestAlgorithm": "sha256",
                "timestampUrl": TIMESTAMP_URL,
            }
        }
    }


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(__doc__)
        return 2
    try:
        config = signing_config(argv[1])
    except ValueError as exc:
        print(exc, file=sys.stderr)
        return 1
    Path(argv[2]).write_text(json.dumps(config, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
