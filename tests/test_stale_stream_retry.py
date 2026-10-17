"""A stream address that ffmpeg cannot read must be replaced by a genuinely fresh lookup.

The player's retry paths say "the address may be what is wrong: look it up again", but they go
through Resolver.resolve(), which answers from its short-lived cache. Without these tests'
fix a retry received the very same dead address (and the fast, JS-less first attempt that
produced it was never bypassed), so one bad first answer from YouTube failed every retry.

The resolver here is the real one with a scripted backend; only the decoder processes are stand-ins.
"""

from __future__ import annotations

import asyncio
import shutil
import time
from collections.abc import Awaitable, Callable
from types import SimpleNamespace
from typing import Any

import pytest

from twitch_radio import player as player_mod
from twitch_radio.extraction import Resolver
from twitch_radio.player import QueuedRequest, RadioPlayer

PAGE = "https://www.youtube.com/watch?v=abcdefghijk"
DEAD = "https://rr1.googlevideo.example/dead"
GOOD = "https://rr1.googlevideo.example/good"


def run(coro: Awaitable[Any]) -> Any:
    return asyncio.run(asyncio.wait_for(coro, timeout=20))  # type: ignore[arg-type]


async def until(predicate: Callable[[], bool], timeout: float = 5.0) -> None:
    deadline = time.monotonic() + timeout
    while not predicate():
        assert time.monotonic() < deadline, "condition not reached in time"
        await asyncio.sleep(0.01)


def fast_attempt(options: dict[str, Any]) -> bool:
    clients = options.get("extractor_args", {}).get("youtube", {}).get("player_client")
    return clients == ["visionos"]


class ScriptedBackend:
    """The fast (visionos) attempt 'succeeds' with an address that does not work; the full attempt
    returns one that does. This is the shape of a YouTube client whose URLs ffmpeg is refused."""

    def __init__(self) -> None:
        self.calls: list[tuple[str, bool]] = []

    async def extract(self, query: str, options: dict[str, Any], timeout: float) -> dict[str, Any]:
        fast = fast_attempt(options)
        self.calls.append((query, fast))
        return {
            "id": "abcdefghijk",
            "title": "Song",
            "webpage_url": PAGE,
            "url": DEAD if fast else GOOD,
            "uploader": "Artist",
            "duration": 200,
            "http_headers": {"User-Agent": "test"},
        }

    async def aclose(self) -> None:
        return None

    def status(self) -> dict[str, Any]:
        return {}


def make_resolver() -> tuple[Resolver, ScriptedBackend]:
    settings = SimpleNamespace(
        ytdlp_concurrency=1,
        ytdlp_worker_idle_seconds=60,
        ytdlp_player_client=(),
        ytdlp_js_runtime_path=None,
        ytdlp_js_runtime_name="deno",
        ytdlp_cookies_file=None,
        ytdlp_pot_provider_url=None,
        ytdlp_cache_ttl_seconds=900,
        ytdlp_extract_timeout_seconds=45,
        log_level="INFO",
    )
    resolver = Resolver(settings)  # type: ignore[arg-type]
    backend = ScriptedBackend()
    resolver._backend = backend  # type: ignore[assignment]
    return resolver, backend


class DeadStdout:
    async def read(self, n: int = -1) -> bytes:
        return b""  # the decoder ended without a single sample: a 403, an expired address


class LiveStdout:
    def __init__(self, decoder: Decoder) -> None:
        self._decoder = decoder

    async def read(self, n: int = -1) -> bytes:
        if self._decoder.killed:
            return b""
        return b"\x01" * 64


class Decoder:
    def __init__(self, url: str) -> None:
        self.url = url
        self.killed = False
        self.returncode: int | None = None if url == GOOD else 1
        self.stdout: Any = LiveStdout(self) if url == GOOD else DeadStdout()

    def kill(self) -> None:
        self.killed = True
        self.returncode = -9

    async def wait(self) -> int | None:
        return self.returncode


@pytest.fixture(autouse=True)
def _pretend_ffmpeg_exists(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(shutil, "which", lambda name, *a, **k: f"/usr/bin/{name}")


def test_invalidate_makes_the_next_resolve_a_full_fresh_lookup() -> None:
    async def scenario() -> None:
        resolver, backend = make_resolver()
        first = await resolver.resolve(PAGE, requester_id=1)
        assert first is not None and first.stream_url == DEAD
        assert (await resolver.resolve(PAGE, requester_id=1)) is first or len(backend.calls) == 1  # cached

        resolver.invalidate(first.webpage_url)

        second = await resolver.resolve(PAGE, requester_id=1)
        assert second is not None and second.stream_url == GOOD
        assert backend.calls[-1][1] is False  # the full attempt, not the fast one again

    run(scenario())


def test_a_dead_first_address_is_replaced_before_the_request_is_given_up_on(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Preparation ahead of time: the decoder gets no audio from the first address, so the retry
    has to come back with the second one and the song becomes ready."""

    async def scenario() -> None:
        resolver, backend = make_resolver()
        spawned: list[Decoder] = []

        async def fake_spawn(*args: str, **kwargs: Any) -> Decoder:
            decoder = Decoder(args[args.index("-i") + 1])
            spawned.append(decoder)
            return decoder

        monkeypatch.setattr(player_mod, "_spawn", fake_spawn)
        monkeypatch.setattr(player_mod, "_PREP_RETRY_DELAYS", (0.01, 0.01))
        player = RadioPlayer(resolver=resolver.resolve, audio_bitrate_kbps=96)
        player.set_track_invalidator(resolver.invalidate)
        notices: list[str] = []

        async def notify(message: str) -> None:
            notices.append(message)

        player.set_track_failure_notifier(notify)
        # Something is 'playing', so the queued song is prepared ahead rather than started at once.
        player._current_decoder = Decoder(GOOD)  # type: ignore[assignment]
        player._active_request = QueuedRequest(webpage_url="x", requester_id=1, requester_name="v")
        player._prep_task = asyncio.create_task(player._prep_loop())
        try:
            player.enqueue(QueuedRequest(webpage_url=PAGE, requester_id=7, requester_name="viewer"))
            await until(lambda: any(player.is_ready(r) for r in player.queued_items()), timeout=8)
            assert [d.url for d in spawned][:2] == [DEAD, GOOD]
            assert backend.calls[0][1] is True and backend.calls[-1][1] is False
            assert not notices  # nobody was told the song failed: it did not
        finally:
            await player.stop()

    run(scenario())
