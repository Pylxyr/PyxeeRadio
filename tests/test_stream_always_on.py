"""/stream.opus is a live stream for as long as the bot runs - silence when nothing plays - and
what the overlay shows follows what that stream actually delivers.

Real ffmpeg, no songs: everything here is about the stream itself. Skipped when ffmpeg isn't installed.
"""

from __future__ import annotations

import asyncio
import math
import shutil
import socket
import subprocess
import time
from pathlib import Path
from types import SimpleNamespace
from typing import Any

import aiohttp
import pytest

from twitch_radio import player as player_mod
from twitch_radio.admin.app import run_admin_server
from twitch_radio.models import Track
from twitch_radio.player import NowPlaying, RadioPlayer, _iter_ogg_pages, _ogg_resync, _opus_pre_skip
from twitch_radio.store import JsonStore

needs_ffmpeg = pytest.mark.skipif(shutil.which("ffmpeg") is None, reason="ffmpeg not installed")


def run(coro: Any, timeout: float = 60) -> Any:
    return asyncio.run(asyncio.wait_for(coro, timeout=timeout))


async def no_resolve(query: str, requester_id: int) -> Track | None:
    return None


def make_player(**kwargs: Any) -> RadioPlayer:
    return RadioPlayer(resolver=no_resolve, audio_bitrate_kbps=96, **kwargs)


async def until(predicate: Any, timeout: float = 10.0) -> None:
    deadline = time.monotonic() + timeout
    while not predicate():
        if time.monotonic() > deadline:
            raise AssertionError("condition not reached in time")
        await asyncio.sleep(0.02)


def decode_seconds(ogg: bytes, directory: Path) -> float:
    path = directory / "capture.ogg"
    path.write_bytes(ogg)
    pcm = subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-nostdin",
            "-i",
            str(path),
            "-f",
            "s16le",
            "-ar",
            "48000",
            "-ac",
            "2",
            "-",
        ],
        capture_output=True,
        check=True,
    ).stdout
    return len(pcm) / (48000 * 2 * 2)


# -- the stream is up without anyone having asked for it -------------------------------------------


@needs_ffmpeg
def test_encoder_runs_with_no_listener_and_no_song() -> None:
    async def scenario() -> None:
        player = make_player()
        player.start()
        try:
            await until(lambda: player.encoder_running)
            assert player.listener_count == 0
            await until(lambda: player.ogg_header_snapshot() != b"")
            assert player.ogg_header_snapshot().startswith(b"OggS")
            assert b"OpusHead" in player.ogg_header_snapshot()
            assert player.encoder_starts == 1
        finally:
            await player.stop()

    run(scenario())


@needs_ffmpeg
def test_first_listener_gets_the_header_and_live_silence_at_once(tmp_path: Path) -> None:
    async def scenario() -> None:
        player = make_player()
        player.start()
        try:
            await until(lambda: player.ogg_header_snapshot() != b"")
            started = time.monotonic()
            queue = player.subscribe()
            received = bytearray(player.ogg_header_snapshot())
            first_page_after = None
            while time.monotonic() - started < 3.0:
                received += await asyncio.wait_for(queue.get(), timeout=2)
                if first_page_after is None and len(received) > len(player.ogg_header_snapshot()):
                    first_page_after = time.monotonic() - started
            assert first_page_after is not None and first_page_after < 0.5, first_page_after
            assert bytes(received[:4]) == b"OggS"
            # ~3 s of silence arrived as a stream of pages, not as one lump a second: the old
            # one-page-a-second muxer would have delivered only two or three.
            pages, rest = _iter_ogg_pages(bytes(received))
            assert rest == b""
            assert len(pages) > 15, len(pages)
            player.unsubscribe(queue)
        finally:
            await player.stop()
        seconds = decode_seconds(bytes(received), tmp_path)
        assert 2.5 < seconds < 3.8, seconds  # real time: no burst, no drift

    run(scenario())


@needs_ffmpeg
def test_first_bytes_follow_player_start_closely() -> None:
    """Used to take ~3 s: ffmpeg probed its input for a second, and the feed loop slept through an
    idle tick because nothing told it the encoder had started."""

    async def scenario() -> None:
        player = make_player()
        queue = player.subscribe()
        started = time.monotonic()
        player.start()
        try:
            await asyncio.wait_for(queue.get(), timeout=5)
            assert time.monotonic() - started < 2.0, time.monotonic() - started
        finally:
            await player.stop()

    run(scenario())


@needs_ffmpeg
def test_lazy_encoder_also_starts_producing_at_once(monkeypatch: pytest.MonkeyPatch) -> None:
    """keep_encoder_running=False: the first listener starts the encoder, and the feed loop (which was
    idling because there was no encoder to write to) is woken rather than left to its next tick."""
    monkeypatch.setattr(player_mod, "_IDLE_TICK_SECONDS", 30.0)  # a missed wake-up would now show

    async def scenario() -> None:
        player = make_player(keep_encoder_running=False)
        player.start()
        try:
            await asyncio.sleep(0.3)
            assert not player.encoder_running
            started = time.monotonic()
            queue = player.subscribe()
            await asyncio.wait_for(queue.get(), timeout=5)  # header pages
            received = bytearray()
            while len(received) < 100:
                received += await asyncio.wait_for(queue.get(), timeout=5)  # audio
            assert time.monotonic() - started < 2.0, time.monotonic() - started
        finally:
            await player.stop()

    run(scenario())


# -- what listeners are handed ---------------------------------------------------------------------


@needs_ffmpeg
def test_listeners_only_ever_get_whole_pages_and_late_joiners_a_decodable_start(tmp_path: Path) -> None:
    async def scenario() -> None:
        player = make_player()
        player.start()
        try:
            early = player.subscribe()
            await asyncio.sleep(1.5)
            late = player.subscribe()
            header = player.ogg_header_snapshot()
            assert header
            late_bytes = bytearray(header)
            for _ in range(15):
                item = await asyncio.wait_for(late.get(), timeout=5)
                pages, rest = _iter_ogg_pages(item)
                assert rest == b"" and len(pages) == 1, "each item is exactly one Ogg page"
                late_bytes += item
            assert late_bytes.count(b"OpusHead") == 1
            assert not early.empty()
            player.unsubscribe(late)
            player.unsubscribe(early)
        finally:
            await player.stop()
        assert decode_seconds(bytes(late_bytes), tmp_path) > 1.0

    run(scenario())


def test_ogg_resync_drops_what_precedes_the_next_page_boundary() -> None:
    assert _ogg_resync(b"OggS" + b"x" * 40) == b"OggS" + b"x" * 40  # nothing wrong: untouched
    assert _ogg_resync(b"junkjunkOggSpayload") == b"OggSpayload"
    assert _ogg_resync(b"junk") == b"unk"  # "OggS" could be split across reads: keep the last 3 bytes
    assert _ogg_resync(b"") == b""
    oversized = b"OggS" + b"x" * 70_000 + b"OggSnext"
    assert _ogg_resync(oversized) == b"OggSnext"


@needs_ffmpeg
def test_pre_skip_is_read_from_the_header() -> None:
    async def scenario() -> None:
        player = make_player()
        player.start()
        try:
            await until(lambda: player.ogg_header_snapshot() != b"")
            assert 0 < _opus_pre_skip(player.ogg_header_snapshot()) < 4800
        finally:
            await player.stop()

    run(scenario())
    assert _opus_pre_skip(b"OggS-not-an-opus-header") == 0


# -- the real endpoint ----------------------------------------------------------------------------


def free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


@needs_ffmpeg
def test_stream_endpoint_answers_at_once_with_silence_and_healthz_reports_the_stream(tmp_path: Path) -> None:
    async def scenario() -> None:
        player = make_player()
        player.start()
        port = free_port()
        runner = await run_admin_server(
            player=player,
            tunables_store=JsonStore(tmp_path / "tunables.json"),
            toggles_store=JsonStore(tmp_path / "toggles.json"),
            broadcast_info={},
            port=port,
        )
        try:
            async with aiohttp.ClientSession() as http:
                started = time.monotonic()
                async with http.get(f"http://127.0.0.1:{port}/stream.opus") as resp:
                    assert resp.status == 200
                    assert resp.headers["Content-Type"] == "audio/ogg"
                    assert resp.headers["Accept-Ranges"] == "none"
                    got = bytearray()
                    first = None
                    async for chunk in resp.content.iter_any():
                        got += chunk
                        if first is None:
                            first = time.monotonic() - started
                        if time.monotonic() - started > 1.5:
                            break
                    assert first is not None and first < 1.5, first
                    assert bytes(got[:4]) == b"OggS" and b"OpusHead" in bytes(got[:200])
                    async with http.get(f"http://127.0.0.1:{port}/healthz") as health:
                        stream = (await health.json())["stream"]
                    assert stream["encoder_running"] is True
                    assert stream["listeners"] == 1
                    assert 0.0 <= stream["lead_seconds"] < 1.0
        finally:
            await player.stop()
            await runner.cleanup()

    run(scenario())


# -- an encoder that dies -------------------------------------------------------------------------


@needs_ffmpeg
def test_listeners_are_ended_cleanly_when_the_encoder_dies_and_the_stream_comes_back(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(player_mod, "_MIN_BACKOFF", 0.3)

    async def scenario() -> None:
        player = make_player()
        player.start()
        try:
            await until(lambda: player.ogg_header_snapshot() != b"")
            queue = player.subscribe()
            await asyncio.wait_for(queue.get(), timeout=5)
            assert player._encoder is not None
            player._encoder.kill()
            # The connection gets a proper end of stream (the empty sentinel handle_stream() stops on)...
            while True:
                if not await asyncio.wait_for(queue.get(), timeout=10):
                    break
            assert player.listener_count == 0
            # ...and a listener that reconnects finds a fresh encoder with its own header.
            await until(
                lambda: player.encoder_starts == 2 and player.ogg_header_snapshot() != b"", timeout=15
            )
            again = player.subscribe()
            assert player.ogg_header_snapshot().startswith(b"OggS")
            assert await asyncio.wait_for(again.get(), timeout=5)
            player.unsubscribe(again)
        finally:
            await player.stop()

    run(scenario())


# -- the overlay follows the stream, not just the clock --------------------------------------------


def song(title: str) -> NowPlaying:
    return NowPlaying(
        title=title,
        uploader="u",
        thumbnail_url=None,
        requester_name="viewer",
        requester_id=7,
        webpage_url=f"https://youtu.be/{title}",
        started_at=time.monotonic(),
        duration=200,
    )


def running_player(**kwargs: Any) -> RadioPlayer:
    """A player that believes an encoder is running, for tests that feed it pages by hand."""
    player = make_player(**kwargs)
    player._encoder = SimpleNamespace(returncode=None)  # type: ignore[assignment]
    return player


def test_a_change_is_audible_when_the_page_carrying_it_goes_out_plus_the_listener_buffer() -> None:
    async def scenario() -> None:
        player = running_player(overlay_delay_seconds=2.0)
        player._encoder_fed_seconds = 100.0
        player._set_now_playing(song("a"))
        event = player._audible_events[-1]
        assert event.position == 100.0 and event.due == math.inf, "waits for the stream, not the clock"
        assert player.audible_view().now is None

        player._resolve_audible_events(start=99.7, end=99.9, wall=500.0)  # an earlier page: not it
        assert event.due == math.inf
        player._resolve_audible_events(start=99.9, end=100.1, wall=501.0)  # the page that holds 100.0
        # The page went out at 501.0; position 100.0 is 0.1 s into it; the listener's buffer adds 2 s.
        assert event.due == pytest.approx(501.0 + 0.1 + 2.0)

    run(scenario())


def test_changes_are_audible_in_order_and_never_early() -> None:
    async def scenario() -> None:
        player = running_player(overlay_delay_seconds=1.0)
        player._encoder_fed_seconds = 10.0
        player._set_now_playing(song("a"))
        player._encoder_fed_seconds = 20.0
        player._set_now_playing(None)
        first, second = player._audible_events
        player._resolve_audible_events(start=9.9, end=10.1, wall=100.0)
        assert first.due == pytest.approx(101.1) and second.due == math.inf
        # The second change goes out in a page that arrived before the first one's moment came
        # (a catch-up burst): it still may not be heard before it.
        player._resolve_audible_events(start=19.9, end=20.1, wall=100.2)
        assert second.due >= first.due

    run(scenario())


def test_a_gap_in_a_song_waits_for_the_stream_like_any_other_change() -> None:
    async def scenario() -> None:
        player = running_player(overlay_delay_seconds=0.0)
        player._encoder_fed_seconds = 30.0
        player._note_gap(0.4)
        event = player._audible_events[-1]
        assert event.gap == 0.4 and event.position == 30.0 and event.due == math.inf

    run(scenario())


def test_changes_the_encoder_never_delivers_are_not_waited_for_forever() -> None:
    async def scenario() -> None:
        player = running_player(overlay_delay_seconds=1.0)
        player._encoder_fed_seconds = 5.0
        player._set_now_playing(song("a"))
        event = player._audible_events[-1]
        assert player.audible_view().now is None and event.due == math.inf
        event.created -= player_mod._MAX_ENCODER_LEAD_SECONDS + 0.5  # it has been waiting too long
        player.audible_view()
        assert event.due == pytest.approx(event.created + 1.0)

    run(scenario())


def test_an_encoder_restart_releases_everything_waiting_on_the_old_one() -> None:
    async def scenario() -> None:
        player = running_player(overlay_delay_seconds=0.0)
        player._encoder_fed_seconds = 5.0
        player._set_now_playing(song("a"))
        player._reset_ogg_state()
        assert player._audible_events[-1].due != math.inf

    run(scenario())


def test_without_an_encoder_the_timeline_runs_on_the_clock() -> None:
    async def scenario() -> None:
        player = make_player(overlay_delay_seconds=0.5)
        before = time.monotonic()
        player._set_now_playing(song("a"))
        due = player._audible_events[-1].due
        assert before + 0.5 <= due <= time.monotonic() + 0.5 + 0.01

    run(scenario())


def test_no_encoder_or_no_output_yet_means_no_lead() -> None:
    async def scenario() -> None:
        player = make_player(overlay_delay_seconds=1.0)
        player._encoder_fed_seconds = 50.0
        assert player.stream_lead_seconds == 0.0, "nothing is running"
        player._encoder = SimpleNamespace(returncode=None)  # type: ignore[assignment]
        player._encoder_emitted_seconds = None
        assert player.stream_lead_seconds == 0.0, "nothing has come out yet"
        player._encoder_emitted_seconds = 60.0
        assert player.stream_lead_seconds == 0.0, "never negative"
        player._encoder_emitted_seconds = 1.0
        assert player.stream_lead_seconds == player_mod._MAX_ENCODER_LEAD_SECONDS, "a stuck encoder is capped"

    run(scenario())


@needs_ffmpeg
def test_measured_lead_of_a_healthy_encoder_is_a_fraction_of_a_second() -> None:
    async def scenario() -> None:
        player = make_player()
        player.start()
        try:
            await until(lambda: player._encoder_emitted_seconds is not None)
            samples = []
            for _ in range(15):
                samples.append(player.stream_lead_seconds)
                await asyncio.sleep(0.1)
            # One page is 0.1 s and a frame 0.02 s; allow plenty for a loaded CI machine.
            assert max(samples) < 0.6, samples
        finally:
            await player.stop()

    run(scenario())
