"""Pause, resume and skip: where a song resumes, what the stream does around a cut, and how the
audio is fed (one shared clock, a read-ahead buffer, headers the CDN insists on).

Real ffmpeg and a real local HTTP server; skipped when ffmpeg isn't installed.
"""

from __future__ import annotations

import array
import asyncio
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path
from types import SimpleNamespace
from typing import Any

import pytest
from aiohttp import web
from aiohttp.test_utils import TestServer

from twitch_radio import player as player_mod
from twitch_radio.extraction import _clean_headers
from twitch_radio.extractor_worker import _ffmpeg_cookies, _project
from twitch_radio.models import Track
from twitch_radio.player import (
    _BYTES_PER_SECOND,
    _CHUNK_BYTES,
    _CHUNK_DURATION,
    NowPlaying,
    QueuedRequest,
    RadioPlayer,
    _decoder_cmd,
    _PcmSource,
)

FFMPEG = os.environ.get("FIXTURE_FFMPEG", "ffmpeg")
needs_ffmpeg = pytest.mark.skipif(shutil.which(FFMPEG) is None, reason="ffmpeg not installed")

UA = "Mozilla/5.0 (a client the CDN knows)"


def run(coro: Any, timeout: float = 90) -> Any:
    return asyncio.run(asyncio.wait_for(coro, timeout=timeout))


async def until(predicate: Any, timeout: float = 15.0) -> None:
    deadline = time.monotonic() + timeout
    while not predicate():
        if time.monotonic() > deadline:
            raise AssertionError("condition not reached in time")
        await asyncio.sleep(0.02)


def make_song(path: Path, seconds: int, hz: int = 440) -> None:
    subprocess.run(
        [FFMPEG, "-hide_banner", "-loglevel", "error", "-y", "-nostdin", "-f", "lavfi", "-i",
         f"sine=f={hz}:d={seconds}:r=48000", "-ac", "2", "-c:a", "libopus", "-b:a", "96k", "-f", "ogg", str(path)],
        check=True,
    )  # fmt: skip


def make_two_tone_song(path: Path, seconds: int, switch_at: float) -> None:
    """440 Hz until `switch_at` seconds into the song, 880 Hz after: where a stream is in the song
    can be read off the pitch."""
    expr = f"0.4*sin(2*PI*if(lt(t,{switch_at}),440,880)*t)"
    subprocess.run(
        [FFMPEG, "-hide_banner", "-loglevel", "error", "-y", "-nostdin", "-f", "lavfi", "-i",
         f"aevalsrc='{expr}':s=48000:d={seconds}", "-ac", "2", "-c:a", "libopus", "-b:a", "96k", "-f", "ogg", str(path)],
        check=True,
    )  # fmt: skip


def tone_timeline(ogg: bytes, directory: Path) -> list[tuple[float, int]]:
    """(seconds into the capture, pitch) per 100 ms: 0 for silence, else 440 or 880 (by counting
    zero crossings, which is all two well-separated tones need)."""
    path = directory / "tones.ogg"
    path.write_bytes(ogg)
    pcm = subprocess.run(
        [FFMPEG, "-v", "error", "-i", str(path), "-f", "s16le", "-ar", "48000", "-ac", "1", "-"],
        capture_output=True, check=True,
    ).stdout  # fmt: skip
    samples = array.array("h")
    samples.frombytes(pcm[: len(pcm) // 2 * 2])
    window = 4800
    out: list[tuple[float, int]] = []
    for start in range(0, len(samples) - window, window):
        block = samples[start : start + window]
        if max(max(block), -min(block)) < 1500:
            out.append((start / 48000, 0))
            continue
        crossings = sum(1 for a, b in zip(block, block[1:], strict=False) if (a < 0) != (b < 0))
        hz = crossings / 2 / (window / 48000)
        out.append((start / 48000, 440 if hz < 660 else 880))
    return out


def request(name: str, **kwargs: Any) -> QueuedRequest:
    return QueuedRequest(
        webpage_url=f"https://youtu.be/{name}", requester_id=1, requester_name="viewer", title=name, **kwargs
    )


class Media:
    """A local CDN: serves a directory (with Range requests), logs what it was asked, and can insist
    on request headers."""

    def __init__(self, directory: Path) -> None:
        self.directory = directory
        self.requests: list[dict[str, str]] = []
        self.required_headers: dict[str, str] = {}
        self.server: TestServer | None = None

    async def __aenter__(self) -> Media:
        app = web.Application()
        app.router.add_get("/media/{name}", self._serve)
        self.server = TestServer(app)
        await self.server.start_server()
        return self

    async def __aexit__(self, *exc: object) -> None:
        assert self.server is not None
        await self.server.close()

    async def _serve(self, request: web.Request) -> web.StreamResponse:
        self.requests.append(dict(request.headers))
        for name, value in self.required_headers.items():
            if name == "Cookie":
                if value not in request.headers.get("Cookie", ""):
                    return web.Response(status=403, text="forbidden")
            elif request.headers.get(name) != value:
                return web.Response(status=403, text="forbidden")
        path = self.directory / request.match_info["name"]
        return web.FileResponse(path)

    def track(self, name: str, seconds: int, **kwargs: Any) -> Track:
        assert self.server is not None
        return Track(
            title=name,
            webpage_url=f"https://youtu.be/{name}",
            stream_url=f"http://127.0.0.1:{self.server.port}/media/{name}",
            uploader="test",
            duration=seconds,
            requester_id=1,
            **kwargs,
        )


def make_player(tracks: dict[str, Track], calls: list[str] | None = None, **kwargs: Any) -> RadioPlayer:
    async def resolver(query: str, requester_id: int) -> Track | None:
        if calls is not None:
            calls.append(query)
        return tracks.get(query.rsplit("/", 1)[-1])

    kwargs.setdefault("prefetch_enabled", False)
    kwargs.setdefault("overlay_delay_seconds", 0.0)
    return RadioPlayer(resolver=resolver, audio_bitrate_kbps=96, **kwargs)


# -- headers the CDN insists on ------------------------------------------------------------------


def test_clean_headers_keeps_plain_strings_and_refuses_anything_that_could_add_a_header() -> None:
    raw = {
        "User-Agent": UA,
        "Accept": "*/*",
        "Cookie": "sid=1",  # travels as -cookies, never as a header
        "X-Evil": "ok\r\nX-Injected: 1",
        "Bad\nName": "v",
        "Has:Colon": "v",
        " Padded": "v",
        "": "v",
        "Number": 5,
        5: "five",
    }
    assert _clean_headers(raw) == {"User-Agent": UA, "Accept": "*/*"}
    assert _clean_headers(None) == {}
    assert _clean_headers(["not", "a", "dict"]) == {}


def test_decoder_command_forwards_headers_and_cookies_before_the_input() -> None:
    plain = _decoder_cmd("http://x/y")
    assert "-headers" not in plain and "-cookies" not in plain and "-ss" not in plain
    cmd = _decoder_cmd(
        "http://x/y",
        headers={"User-Agent": UA, "Accept-Language": "en"},
        cookies="a=1; path=/; domain=.x;\r\n",
        start_at=61.2345,
    )
    at = cmd.index("-i")
    assert cmd[cmd.index("-headers") + 1] == f"User-Agent: {UA}\r\nAccept-Language: en\r\n"
    assert cmd[cmd.index("-cookies") + 1] == "a=1; path=/; domain=.x;\r\n"
    assert cmd[cmd.index("-ss") + 1] == "61.234" or cmd[cmd.index("-ss") + 1] == "61.235"
    assert max(cmd.index("-headers"), cmd.index("-cookies"), cmd.index("-ss")) < at, "all are input options"
    assert cmd[at + 1] == "http://x/y"


def test_worker_projection_carries_headers_and_cookies_for_the_item_and_its_entries() -> None:
    class Cookie:
        def __init__(self, name: str, value: str) -> None:
            self.name, self.value, self.path, self.domain = name, value, "/", ".example.com"

    ydl = SimpleNamespace(cookiejar=SimpleNamespace(get_cookies_for_url=lambda url: [Cookie("sid", "1")]))
    info = {
        "url": "https://cdn.example.com/a",
        "title": "t",
        "http_headers": {"User-Agent": UA},
        "formats": ["dropped"],
        "entries": [
            {"url": "https://cdn.example.com/b", "http_headers": {"User-Agent": UA}},
            {"url": "ytsearch:x"},
        ],
    }
    out = _project(info, ydl)
    assert out["http_headers"] == {"User-Agent": UA}
    assert out["http_cookies"] == "sid=1; path=/; domain=.example.com;\r\n"
    assert "formats" not in out
    first, second = out["entries"]
    assert first["http_headers"] == {"User-Agent": UA} and first["http_cookies"].startswith("sid=1")
    assert "http_cookies" not in second, "not an http(s) address: nothing to attach"
    assert "http_cookies" not in _project(info), "no yt-dlp instance, no cookies"

    broken = SimpleNamespace(cookiejar=SimpleNamespace(get_cookies_for_url=lambda url: 1 / 0))
    assert _ffmpeg_cookies(broken, "https://x/") == "", "a cookie problem must not cost the whole resolve"


@needs_ffmpeg
def test_a_cdn_that_insists_on_yt_dlps_headers_is_only_served_when_they_are_forwarded(tmp_path: Path) -> None:
    make_song(tmp_path / "a", 20)

    async def scenario() -> None:
        async with Media(tmp_path) as media:
            media.required_headers = {"User-Agent": UA, "X-Client": "web", "Cookie": "sid=1"}
            good = media.track(
                "a",
                20,
                http_headers={"User-Agent": UA, "X-Client": "web"},
                http_cookies="sid=1; path=/;\r\n",  # (no domain: ffmpeg matches it against host:port)
            )
            player = make_player({"a": good})
            player.start()
            try:
                player.enqueue(request("a"))
                await until(lambda: player.now_playing is not None)
            finally:
                await player.stop()
            assert media.requests and all(r.get("User-Agent") == UA for r in media.requests)

    run(scenario())


@needs_ffmpeg
def test_a_song_the_cdn_refuses_is_reported_not_announced_as_playing(tmp_path: Path) -> None:
    make_song(tmp_path / "a", 20)

    async def scenario() -> None:
        async with Media(tmp_path) as media:
            media.required_headers = {"User-Agent": UA}
            refused = media.track("a", 20)  # no headers forwarded: what used to happen
            player = make_player({"a": refused})
            messages: list[str] = []

            async def notifier(message: str) -> None:
                messages.append(message)

            player.set_track_failure_notifier(notifier)
            player.start()
            try:
                player.enqueue(request("a"))
                await until(lambda: any("couldn't be played" in m for m in messages))
                assert player.now_playing is None
                assert not any("Now Playing" in m or "is Now Playing" in m for m in messages)
                await until(lambda: player.queue_size() == 0 and player.state.value != "playing")
            finally:
                await player.stop()

    run(scenario())


# -- pause and resume ------------------------------------------------------------------------------


def _playing_player(**kwargs: Any) -> tuple[RadioPlayer, Track]:
    track = Track(
        title="t", webpage_url="https://youtu.be/t", stream_url="http://127.0.0.1:1/t", uploader="u",
        duration=200, requester_id=1,
    )  # fmt: skip
    player = make_player({"t": track}, **kwargs)
    active = request("t")
    player._active_request = active
    player._active_track = track
    player._active_track_at = time.monotonic()
    player._active_start_offset = 30.0
    player._played_bytes = int(12.5 * _BYTES_PER_SECOND)
    player._now_playing = NowPlaying(
        title="t", uploader="u", thumbnail_url=None, requester_name="viewer", requester_id=1,
        webpage_url=track.webpage_url, started_at=time.monotonic(), duration=200, start_offset=30.0,
    )  # fmt: skip
    return player, track


def test_pause_remembers_exactly_where_the_song_was_and_its_track() -> None:
    async def scenario() -> None:
        player, track = _playing_player()
        assert player.pause() is True
        resumed = player._pending[0]
        assert resumed.start_at == pytest.approx(42.5), "offset it began at + audio handed to the encoder"
        assert resumed.resume_track is track and resumed.resume_track_at == player._active_track_at
        assert resumed.bypass_listener_pause is True

    run(scenario())


def test_a_song_paused_in_its_last_second_does_not_come_back() -> None:
    async def scenario() -> None:
        player, _ = _playing_player()
        player._played_bytes = int(169.5 * _BYTES_PER_SECOND)  # 30 + 169.5 of 200
        player.pause()
        assert player._pending == []

    run(scenario())


def test_a_song_that_was_just_skipped_is_not_resurrected_by_a_pause() -> None:
    async def scenario() -> None:
        player, _ = _playing_player()
        decoder = SimpleNamespace(returncode=None, kill=lambda: None)
        player._current_decoder = decoder  # type: ignore[assignment]
        player._cut = decoder  # type: ignore[assignment]  # skip() has cut it; it is winding down
        player.pause()
        assert player._pending == []

    run(scenario())


def test_a_song_paused_before_it_began_starts_where_it_was_going_to() -> None:
    async def scenario() -> None:
        player, track = _playing_player()
        player._now_playing = None
        player._active_request = request("t", start_at=12.0, resume_track=track, resume_track_at=5.0)
        player.pause()
        resumed = player._pending[0]
        assert resumed.start_at == 12.0 and resumed.resume_track is track and resumed.resume_track_at == 5.0

    run(scenario())


@needs_ffmpeg
def test_resume_carries_on_from_the_pause_point_without_asking_yt_dlp_again(tmp_path: Path) -> None:
    make_two_tone_song(tmp_path / "a", 30, switch_at=5.0)

    async def scenario() -> tuple[bytes, float, float]:
        async with Media(tmp_path) as media:
            calls: list[str] = []
            player = make_player({"a": media.track("a", 30)}, calls)
            messages: list[str] = []

            async def notifier(message: str) -> None:
                messages.append(message)

            player.set_track_failure_notifier(notifier)
            player.start()
            captured = bytearray()
            queue = player.subscribe()
            captured += player.ogg_header_snapshot()

            async def drain() -> None:
                while True:
                    captured.extend(await queue.get())

            task = asyncio.create_task(drain())
            try:
                player.enqueue(request("a"))
                await until(lambda: player.now_playing is not None)
                await asyncio.sleep(2.5)
                assert player.pause() is True
                paused_at = player._pending[0].start_at
                assert 1.8 < paused_at < 3.5, paused_at
                await until(lambda: player.now_playing is None)
                await asyncio.sleep(1.0)
                resumed_wall = time.monotonic()
                assert player.resume() is True
                await until(lambda: player.now_playing is not None)
                np = player.now_playing
                assert np is not None and np.start_offset == pytest.approx(paused_at)
                assert calls == ["https://youtu.be/a"], "the resolved track was reused"
                await asyncio.sleep(0.3)
                view = player.audible_view()
                assert view.now is not None and view.elapsed >= paused_at, (view.elapsed, paused_at)
                await asyncio.sleep(4.0)  # long enough to cross the pitch change
                assert sum("Now Playing" in m for m in messages) == 1, "chat is not told again on resume"
                return bytes(captured), paused_at, resumed_wall
            finally:
                task.cancel()
                await player.stop()

    captured, paused_at, _ = run(scenario())
    timeline = tone_timeline(captured, tmp_path)
    # find the resumed part: the first sound after the silence that the pause left
    silent_from = next(i for i, (_, hz) in enumerate(timeline) if hz == 0 and i > 5)
    resumed_from = next(i for i in range(silent_from, len(timeline)) if timeline[i][1] != 0)
    switched = next(i for i in range(resumed_from, len(timeline)) if timeline[i][1] == 880)
    assert timeline[resumed_from][1] == 440, "it did not come back in the middle of the 880 part"
    until_switch = timeline[switched][0] - timeline[resumed_from][0]
    # carried on from where it stopped: the pitch changes 5 s into the song, i.e. (5 - paused_at) later;
    # started over, it would take the whole 5 s
    assert until_switch == pytest.approx(5.0 - paused_at, abs=0.5), (until_switch, paused_at)


@needs_ffmpeg
def test_a_resumed_song_whose_stream_address_has_gone_stale_is_looked_up_again(tmp_path: Path) -> None:
    make_song(tmp_path / "a", 30)

    async def scenario() -> None:
        async with Media(tmp_path) as media:
            assert media.server is not None
            stale = Track(
                title="a", webpage_url="https://youtu.be/a",
                stream_url=f"http://127.0.0.1:{media.server.port}/media/expired",
                uploader="test", duration=30, requester_id=1,
            )  # fmt: skip
            calls: list[str] = []
            player = make_player({"a": media.track("a", 30)}, calls)
            player.start()
            try:
                # as a pause leaves it: the song, how far in it was, and the address it used to play from
                player.enqueue(
                    request("a", start_at=4.0, resume_track=stale, resume_track_at=time.monotonic())
                )
                await until(lambda: player.now_playing is not None)
                np = player.now_playing
                assert np is not None and np.start_offset == pytest.approx(4.0)
                assert calls == ["https://youtu.be/a"], "asked for the address again, once"
            finally:
                await player.stop()

    run(scenario())


# -- skip ------------------------------------------------------------------------------------------


@needs_ffmpeg
def test_skip_hands_the_stream_to_the_next_song_without_a_gap(tmp_path: Path) -> None:
    make_song(tmp_path / "a", 30, 440)
    make_song(tmp_path / "b", 30, 880)

    async def scenario() -> bytes:
        async with Media(tmp_path) as media:
            player = make_player(
                {"a": media.track("a", 30), "b": media.track("b", 30)}, prefetch_enabled=True
            )
            player.start()
            captured = bytearray()
            queue = player.subscribe()
            captured += player.ogg_header_snapshot()

            async def drain() -> None:
                while True:
                    captured.extend(await queue.get())

            task = asyncio.create_task(drain())
            try:
                player.enqueue(request("a"))
                player.enqueue(request("b"))
                await until(lambda: player.now_playing is not None and player.now_playing.title == "a")
                await asyncio.sleep(4.0)
                await until(lambda: player.queue_size() == 1 or True)
                await asyncio.sleep(0.5)
                result = await player.skip()
                assert result.name == "SKIPPED", result
                await until(lambda: player.now_playing is not None and player.now_playing.title == "b")
                await asyncio.sleep(3.0)
            finally:
                task.cancel()
                await player.stop()
            return bytes(captured)

    captured = run(scenario())
    path = tmp_path / "skip.ogg"
    path.write_bytes(captured)
    detect = subprocess.run(
        [FFMPEG, "-hide_banner", "-nostats", "-i", str(path), "-af", "silencedetect=noise=-50dB:d=0.08", "-f", "null", "-"],
        capture_output=True, text=True, check=True,
    ).stderr  # fmt: skip
    starts = [float(m) for m in re.findall(r"silence_start: ([0-9.]+)", detect)]
    ends = [float(m) for m in re.findall(r"silence_end: ([0-9.]+)", detect)]
    total = len(
        subprocess.run(
            [FFMPEG, "-v", "error", "-i", str(path), "-f", "s16le", "-ar", "48000", "-ac", "2", "-"],
            capture_output=True,
            check=True,
        ).stdout  # fmt: skip
    ) / (48000 * 4)
    interior = [(s, e) for s, e in zip(starts, ends, strict=False) if s > 0.5 and e < total - 0.3]
    assert interior == [], f"silence between the skipped song and the next one: {interior}"


# -- the feed ---------------------------------------------------------------------------------------


# Stands in for a decoder whose source goes quiet: one second of audio, 2.5 s of nothing (the CDN
# reopening a connection), then more audio.
_STALLING_DECODER = (
    "import sys, time\n"
    "out = sys.stdout.buffer\n"
    "second = b'\\x10\\x00' * (48000 * 2)\n"
    "out.write(second); out.flush()\n"
    "time.sleep(2.5)\n"
    "for _ in range(6):\n"
    "    out.write(second); out.flush(); time.sleep(0.5)\n"
)


@needs_ffmpeg
def test_a_decoder_that_stalls_is_bridged_with_silence_and_the_overlay_clock_waits(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The CDN stops answering for a couple of seconds mid-song. The stream must carry on in real
    time (silence), not stop - a stopped stream is what leaves a client such as OBS lagging for good -
    and the song's elapsed clock must not count the silence."""
    monkeypatch.setattr(player_mod, "_PCM_READAHEAD_SECONDS", 0.3)
    monkeypatch.setattr(player_mod, "_decoder_cmd", lambda *a, **k: [sys.executable, "-c", _STALLING_DECODER])

    async def scenario() -> None:
        track = Track(
            title="a", webpage_url="https://youtu.be/a", stream_url="http://unused/a", uploader="u",
            duration=60, requester_id=1,
        )  # fmt: skip
        player = make_player({"a": track})
        gaps: list[float] = []
        original = player._note_gap

        def spy(seconds: float) -> None:
            gaps.append(seconds)
            original(seconds)

        player._note_gap = spy  # type: ignore[method-assign]
        player.start()
        started = time.monotonic()
        try:
            player.enqueue(request("a"))
            await until(lambda: player.now_playing is not None)
            await until(lambda: sum(gaps) > 0, timeout=20)
            wall = time.monotonic() - started
            # the length of the stall, less what was already buffered or fed ahead of the clock
            assert 1.0 < sum(gaps) < 3.0, gaps
            # the stream kept up with the clock through the stall instead of stopping for it
            assert player._clock_fed == pytest.approx(wall, abs=1.0), (player._clock_fed, wall)
            # and what the decoder delivered after it is the same song carrying on
            assert player.now_playing is not None
            await asyncio.sleep(0.6)  # the shift is applied once the encoder has put that point on the wire
            view_elapsed = player.audible_view().elapsed
            song_audio = player._played_bytes / _BYTES_PER_SECOND
            assert view_elapsed == pytest.approx(song_audio, abs=0.6), (view_elapsed, song_audio)
        finally:
            await player.stop()

    run(scenario())


def test_pacing_catches_up_after_a_late_chunk_and_only_restarts_after_a_long_stretch() -> None:
    async def scenario() -> None:
        player = RadioPlayer(resolver=lambda q, r: asyncio.sleep(0), audio_bitrate_kbps=96)  # type: ignore[arg-type,return-value]
        # on schedule, a chunk is due LOOKAHEAD early: the first few go out at once, then one per chunk
        started = time.monotonic()
        for _ in range(6):
            await player._pace_wait()
            await player._hand_over(b"\0" * _CHUNK_BYTES)
        took = time.monotonic() - started
        expected = 6 * _CHUNK_DURATION - player_mod._FEED_LOOKAHEAD_SECONDS - _CHUNK_DURATION
        assert took == pytest.approx(expected, abs=0.08), (took, expected)
        # 0.5 s late: not forgiven - the next chunks go out back to back until it is made up
        epoch = player._clock_epoch
        assert epoch is not None
        player._clock_epoch = epoch - 0.5
        before = time.monotonic()
        for _ in range(3):
            await player._pace_wait()
            await player._hand_over(b"\0" * _CHUNK_BYTES)
        assert time.monotonic() - before < 0.05
        assert player._clock_epoch == epoch - 0.5, "the schedule was kept, not reset"
        # a stretch longer than _MAX_PACE_DEBT: a discontinuity, the schedule restarts
        player._clock_epoch = player._clock_epoch - 3.0
        await player._pace_wait()
        assert player._clock_epoch is not None and player._clock_epoch > epoch - 0.5 - 3.0 + 2.0

    run(scenario())


def test_a_cut_wakes_the_pacing_sleep_at_once() -> None:
    async def scenario() -> None:
        player = RadioPlayer(resolver=lambda q, r: asyncio.sleep(0), audio_bitrate_kbps=96)  # type: ignore[arg-type,return-value]
        player._clock_epoch = time.monotonic()
        player._clock_fed = 2.0  # due 1.8 s from now
        asyncio.get_running_loop().call_later(0.05, player._cut_wake.set)
        started = time.monotonic()
        await player._pace_wait(interruptible=True)
        assert time.monotonic() - started < 0.5

    run(scenario())


# -- the read-ahead source -----------------------------------------------------------------------------


class _FakeStream:
    def __init__(self, data: bytes, per_read: int = 7000) -> None:
        self._data = bytearray(data)
        self._per_read = per_read

    async def read(self, n: int) -> bytes:
        await asyncio.sleep(0)
        out = bytes(self._data[: min(n, self._per_read)])
        del self._data[: len(out)]
        return out

    async def readline(self) -> bytes:
        await asyncio.sleep(0)
        out = bytes(self._data)
        self._data.clear()
        return out


class _FakeDecoder:
    def __init__(self, audio: bytes, errors: bytes = b"") -> None:
        self.stdout = _FakeStream(audio)
        self.stderr = _FakeStream(errors)
        self.returncode: int | None = 0
        self.killed = False

    def kill(self) -> None:
        self.killed = True

    async def wait(self) -> int:
        return 0


def test_pcm_source_reads_ahead_in_whole_chunks_and_ends_with_the_short_last_one() -> None:
    async def scenario() -> None:
        audio = bytes(range(256)) * ((2 * _CHUNK_BYTES + 5000) // 256 + 1)
        audio = audio[: 2 * _CHUNK_BYTES + 5000]
        source = _PcmSource(_FakeDecoder(audio, b"boom: it broke\n"))  # type: ignore[arg-type]
        assert await source.wait(1.0)
        await until(lambda: source.eof)
        sizes = []
        got = b""
        while (chunk := source.take()) is not None:
            sizes.append(len(chunk))
            got += chunk
        assert sizes == [_CHUNK_BYTES, _CHUNK_BYTES, 5000]
        assert got == audio
        assert await source.error_text() == "boom: it broke"
        await source.aclose()

    run(scenario())


def test_pcm_source_stops_reading_at_its_limit_and_goes_on_when_room_is_made() -> None:
    async def scenario() -> None:
        source = _PcmSource(_FakeDecoder(b"\1" * (10 * _CHUNK_BYTES)), ahead_seconds=3 * _CHUNK_DURATION)  # type: ignore[arg-type]
        await asyncio.sleep(0.05)
        assert source.buffered_seconds == pytest.approx(3 * _CHUNK_DURATION), "no more than it was allowed"
        assert not source.eof
        source.take()
        await asyncio.sleep(0.05)
        assert source.buffered_seconds == pytest.approx(3 * _CHUNK_DURATION), "refilled to the limit"
        await source.aclose()

    run(scenario())


def test_pcm_source_wait_times_out_when_the_decoder_has_nothing() -> None:
    async def scenario() -> None:
        class Silent(_FakeStream):
            async def read(self, n: int) -> bytes:
                await asyncio.sleep(30)
                return b""

        decoder = _FakeDecoder(b"")
        decoder.stdout = Silent(b"")
        source = _PcmSource(decoder)  # type: ignore[arg-type]
        started = time.monotonic()
        assert await source.wait(0.1) is False
        assert time.monotonic() - started < 1.0
        assert source.take() is None and not source.eof
        await source.aclose()
        assert decoder.killed

    run(scenario())
