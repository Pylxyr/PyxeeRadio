from __future__ import annotations

import re
from dataclasses import dataclass, field


@dataclass(slots=True)
class Track:
    """A resolved, playable track. stream_url is a direct, short-lived
    media URL from yt-dlp — don't hold onto one past Resolver's own cache
    window (YTDLP_CACHE_TTL_SECONDS), since it expires."""

    title: str
    webpage_url: str
    stream_url: str
    uploader: str
    duration: int
    requester_id: int
    thumbnail_url: str | None = None
    query: str = ""
    is_live: bool = False
    # What yt-dlp itself sends when it fetches stream_url (the format's `http_headers`: a browser
    # User-Agent and friends) and the cookies it holds for that address, already formatted the way
    # ffmpeg's `-cookies` wants them. The CDN ties a stream address to the client that asked for it:
    # ffmpeg's own "Lavf/..." User-Agent and no cookies is a 403 for many of them, which is why
    # yt-dlp's own ffmpeg downloader always forwards both (see player._decoder_cmd).
    http_headers: dict[str, str] = field(default_factory=dict)
    http_cookies: str = ""


_YOUTUBE_ID = re.compile(r"[\w-]{11}")


def youtube_thumbnail(entry: dict) -> str | None:  # type: ignore[type-arg]
    """A picture for a yt-dlp entry. Flat results (search, radio mix) carry no `thumbnail`, but a
    YouTube video's picture is always at a predictable address on i.ytimg.com."""
    thumbnail = entry.get("thumbnail")
    if isinstance(thumbnail, str) and thumbnail.startswith("https://"):
        return thumbnail
    video_id = entry.get("id")
    if isinstance(video_id, str) and _YOUTUBE_ID.fullmatch(video_id):
        return f"https://i.ytimg.com/vi/{video_id}/mqdefault.jpg"
    return None
