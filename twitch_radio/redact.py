"""Takes secrets out of text before it is logged, shown in the Logs tab or exported.

Stdlib only, no twitch_radio imports (logbus.py and bot.py use it while logging is being set up).

What leaks in practice: the HTTP libraries under yt-dlp put the whole request target into their
error text ("Max retries exceeded with url: /videoplayback?expire=...&sig=..."), a signed media
address is a bearer credential for as long as it lives, and a Twitch token can turn up in an
exception from the chat library. The video id of a YouTube link is what makes a log line useful
and is not secret, so `v` and `list` survive; every other query parameter does not.
"""

from __future__ import annotations

import re

# Query parameters that are safe, and needed to say which video a line is about.
_KEEP_PARAMS = frozenset({"v", "list"})

# A query string: '?' right after a path or host character, then at least one name=value pair.
_QUERY = re.compile(r"(?<=[\w/.~-])\?[\w%.~-]+=[^\s'\"<>)\]}]*")
# Header-style secrets, and `name=value` secrets outside a URL query (form bodies, repr()s).
_HEADER = re.compile(r"(?i)\b(cookie|set-cookie|authorization|proxy-authorization)\s*[:=]\s*[^\r\n]+")
_SECRET_PAIR = re.compile(
    r"(?i)\b(access_token|refresh_token|client_secret|token|secret|password|sig|signature|lsig|po_token)"
    r"=([^&\s'\"<>]+)"
)
_OAUTH = re.compile(r"(?i)\boauth:[a-z0-9]{10,}")
_BEARER = re.compile(r"(?i)\b(bearer)\s+[a-z0-9._~+/=-]{10,}")


def _filter_query(match: re.Match[str]) -> str:
    pairs = match.group(0)[1:].split("&")
    kept = [p for p in pairs if p.split("=", 1)[0] in _KEEP_PARAMS]
    return ("?" + "&".join(kept)) if kept else "?\u2026"


def redact_text(text: str) -> str:
    """`text` with credentials and signed-address parameters removed. Never raises."""
    try:
        text = _QUERY.sub(_filter_query, text)
        text = _HEADER.sub(lambda m: f"{m.group(1)}: <redacted>", text)
        text = _SECRET_PAIR.sub(lambda m: f"{m.group(1)}=<redacted>", text)
        text = _OAUTH.sub("oauth:<redacted>", text)
        return _BEARER.sub(lambda m: f"{m.group(1)} <redacted>", text)
    except Exception:  # noqa: BLE001 - redaction must never be the reason a log line is lost
        return text


def one_line(text: str, limit: int = 300) -> str:
    """Redacted, on one line, and not longer than `limit` characters."""
    flat = " ".join(redact_text(str(text)).split())
    return flat if len(flat) <= limit else flat[: limit - 1] + "\u2026"
