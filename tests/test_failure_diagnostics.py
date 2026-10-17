"""Why a song failed has to reach the Logs tab and the dashboard, without leaking credentials.

Before these changes: yt-dlp's warnings (a JS solver that could not be downloaded or run, say) were
discarded by `no_warnings`; the reason a queued song could not be prepared was logged at DEBUG only,
so the Logs tab said "could not be prepared (error)" and nothing more; a song that failed after the
dashboard had said "queued" left no trace on screen; and library errors put signed stream addresses
into the log text.
"""

from __future__ import annotations

import asyncio
import io
import logging
import shutil
import sys
import time
from collections.abc import Awaitable, Callable
from types import SimpleNamespace
from typing import Any

import pytest

from twitch_radio import extraction
from twitch_radio import player as player_mod
from twitch_radio.extraction import DownloadError, ProcessBackend, _ExtractionWorker, _WorkerCrashed
from twitch_radio.logbus import LogBus
from twitch_radio.player import QueuedRequest, RadioPlayer
from twitch_radio.redact import one_line, redact_text
from twitch_radio.service import BotRuntime


def run(coro: Awaitable[Any]) -> Any:
    return asyncio.run(asyncio.wait_for(coro, timeout=20))  # type: ignore[arg-type]


async def until(predicate: Callable[[], bool], timeout: float = 5.0) -> None:
    deadline = time.monotonic() + timeout
    while not predicate():
        assert time.monotonic() < deadline, "condition not reached in time"
        await asyncio.sleep(0.01)


# -- redaction ----------------------------------------------------------------------------------------


def test_a_signed_address_loses_its_query_but_a_video_link_keeps_its_id() -> None:
    text = (
        "HTTPSConnectionPool(host='rr1---sn-x.googlevideo.com', port=443): Max retries exceeded with url: "
        "/videoplayback?expire=1&ei=abc&sig=SECRETSIG&lsig=SECRETLSIG (Caused by X)"
    )
    cleaned = redact_text(text)
    assert "SECRETSIG" not in cleaned and "SECRETLSIG" not in cleaned
    assert "/videoplayback?\u2026" in cleaned and "(Caused by X)" in cleaned
    link = "https://www.youtube.com/watch?v=abcdefghijk&list=RDabc&pp=SECRETPP"
    assert redact_text(link) == "https://www.youtube.com/watch?v=abcdefghijk&list=RDabc"
    assert redact_text("plain text, no query") == "plain text, no query"


def test_cookies_tokens_and_bearer_values_are_masked() -> None:
    cleaned = redact_text(
        "Cookie: SID=abc; HSID=def\nAuthorization: Bearer abcdefghij123456\n"
        "failed with oauth:abcdefghijklmnop and access_token=xyz123&x=1"
    )
    for secret in ("SID=abc", "abcdefghij123456", "abcdefghijklmnop", "xyz123"):
        assert secret not in cleaned
    assert one_line("a\n  b\tc", 100) == "a b c"
    assert len(one_line("x" * 1000, 50)) == 50


def test_the_logs_tab_never_receives_a_signed_address_or_one_in_a_traceback() -> None:
    bus = LogBus()
    seen: list[dict[str, Any]] = []
    bus.add_sink(seen.append)
    logger = logging.getLogger("test.redaction")
    logger.propagate = False
    logger.addHandler(bus)
    try:
        try:
            raise RuntimeError("fetch /videoplayback?sig=TRACESECRET failed")
        except RuntimeError:
            logger.exception("lookup failed for https://x.example/a?token=MSGSECRET")
    finally:
        logger.removeHandler(bus)
    joined = "\n".join(e["msg"] for e in seen)
    assert "MSGSECRET" not in joined and "TRACESECRET" not in joined
    assert any(e["msg"].startswith("lookup failed") for e in seen) and any(e.get("cont") for e in seen)


# -- yt-dlp warnings reach the parent ------------------------------------------------------------------


def test_yt_dlp_warnings_were_silent_and_now_reach_stderr(capsys: pytest.CaptureFixture[str]) -> None:
    import yt_dlp

    from twitch_radio.extractor_worker import _Worker

    options = {"quiet": True, "no_warnings": True, "verbose": False}
    yt_dlp.YoutubeDL(dict(options)).report_warning("[youtube] challenge solver script download failed")
    assert capsys.readouterr().err == ""  # the old behaviour: the warning is thrown away

    worker = _Worker(io.StringIO())
    worker._ydl(options).report_warning("[youtube] challenge solver script download failed")
    assert "WARNING: [youtube] challenge solver script download failed" in capsys.readouterr().err


def test_only_the_warnings_that_explain_a_failure_are_promoted() -> None:
    level = extraction.worker_line_level
    assert level("WARNING: [youtube] n challenge solving failed") == logging.WARNING
    assert level("WARNING: [youtube] abc: Sign in to confirm you're not a bot") == logging.WARNING
    assert level("WARNING: [youtube] abc: Some formats have been skipped") == logging.DEBUG
    assert level("ERROR: [youtube] abc: Video unavailable") == logging.DEBUG  # returned to the caller too
    assert level("[ytdlp] Failed to load challenge solver") == logging.WARNING  # the old heuristic stays


# -- the reason a queued song failed is visible -------------------------------------------------------


@pytest.fixture(autouse=True)
def _pretend_ffmpeg_exists(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(shutil, "which", lambda name, *a, **k: f"/usr/bin/{name}")


class Alive:
    returncode: int | None = None

    def kill(self) -> None:
        self.returncode = -9

    async def wait(self) -> int | None:
        return self.returncode


def test_the_cause_of_a_failed_preparation_is_in_the_warning_and_the_dashboard_hears_of_it(
    monkeypatch: pytest.MonkeyPatch, caplog: pytest.LogCaptureFixture
) -> None:
    caplog.set_level(logging.INFO, logger="twitch_radio.player")
    monkeypatch.setattr(player_mod, "_PREP_RETRY_DELAYS", (0.01, 0.01))

    async def resolver(query: str, requester_id: int) -> None:
        raise DownloadError(
            "ERROR: [youtube] abc: Sign in to confirm you're not a bot "
            "(see https://rr1.example/videoplayback?sig=NOTFORLOGS)"
        )

    async def scenario() -> None:
        player = RadioPlayer(resolver=resolver, audio_bitrate_kbps=96)
        notices: list[str] = []

        async def notify(message: str) -> None:
            notices.append(message)

        player.set_track_failure_notifier(notify)
        player._current_decoder = Alive()  # type: ignore[assignment]
        player._active_request = QueuedRequest(webpage_url="x", requester_id=1, requester_name="v")
        player._prep_task = asyncio.create_task(player._prep_loop())
        try:
            player.enqueue(
                QueuedRequest(
                    webpage_url="https://youtu.be/abcdefghijk", requester_id=7, requester_name="Streamer"
                )
            )
            await until(lambda: bool(notices), timeout=10)
            assert not player.queued_items()  # dropped, not stuck
            at, message = player.last_failure or (0.0, "")
            assert message == notices[0] and "Couldn't load Streamer's song" in message
            assert time.time() - at < 30
        finally:
            await player.stop()

    run(scenario())
    dropped = [r.getMessage() for r in caplog.records if "Dropping" in r.getMessage()]
    assert dropped and "Sign in to confirm" in dropped[0]  # the cause, not just "(error)"
    assert "NOTFORLOGS" not in caplog.text


def test_a_decoder_that_cannot_start_says_so_at_warning(
    monkeypatch: pytest.MonkeyPatch, caplog: pytest.LogCaptureFixture
) -> None:
    from twitch_radio.models import Track

    async def resolver(query: str, requester_id: int) -> Track:
        return Track("t", query, "http://127.0.0.1/s", "u", 100, requester_id)

    async def broken_spawn(*args: str, **kwargs: Any) -> Any:
        raise FileNotFoundError(2, "No such file or directory: 'ffmpeg'")

    monkeypatch.setattr(player_mod, "_spawn", broken_spawn)
    caplog.set_level(logging.WARNING, logger="twitch_radio.player")

    async def scenario() -> None:
        player = RadioPlayer(resolver=resolver, audio_bitrate_kbps=96)
        request = QueuedRequest(
            webpage_url="https://youtu.be/abcdefghijk", requester_id=7, requester_name="v"
        )
        track = await resolver(request.webpage_url, 7)
        assert await player._spawn_ahead(request, track) is None

    run(scenario())
    assert any(
        "Could not start the decoder" in r.getMessage() and "ffmpeg" in r.getMessage() for r in caplog.records
    )
    assert all(r.levelno >= logging.WARNING for r in caplog.records)


def test_the_snapshot_carries_the_latest_failure_for_the_dashboard() -> None:
    async def scenario() -> None:
        settings = SimpleNamespace(owner_id="1", nowplaying_port=8098)
        runtime = BotRuntime(settings)  # type: ignore[arg-type]

        async def resolver(query: str, requester_id: int) -> None:
            return None

        runtime._player = RadioPlayer(resolver=resolver, audio_bitrate_kbps=96)
        assert runtime.snapshot()["last_failure"] is None
        await runtime._player._notify_failed("Couldn't load Streamer's song \u2014 skipping it.")
        failure = runtime.snapshot()["last_failure"]
        assert failure["message"].startswith("Couldn't load") and failure["at"] > 0

    run(scenario())


# -- extraction workers are never leaked ----------------------------------------------------------------


def test_a_reply_longer_than_the_reader_limit_recycles_the_worker() -> None:
    async def scenario() -> None:
        reader = asyncio.StreamReader(limit=32)
        reader.feed_data(b"x" * 200 + b"\n")
        worker = _ExtractionWorker(1)
        worker._proc = SimpleNamespace(stdout=reader, returncode=None)  # type: ignore[assignment]
        with pytest.raises(_WorkerCrashed, match="too long"):
            await worker._read_json()

    run(scenario())


def test_an_unexpected_error_inside_a_request_discards_the_worker_instead_of_leaking_it(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class BadWorker:
        pid = 1
        killed = False

        async def request(self, query: str, options: dict[str, Any], timeout: float) -> dict[str, Any]:
            raise OSError("pipe went away in a way nobody planned for")

        async def kill(self) -> None:
            self.killed = True

    async def scenario() -> None:
        backend = ProcessBackend(1, 60)
        worker = BadWorker()
        backend._all.append(worker)  # type: ignore[arg-type]

        async def spawn() -> BadWorker:
            return worker

        monkeypatch.setattr(backend, "_spawn", spawn)
        with pytest.raises(DownloadError, match="unexpectedly"):
            await backend.extract("q", {}, 5.0)
        assert worker.killed and not backend._all and not backend._idle
        await backend.aclose()

    run(scenario())


def test_stdout_of_the_worker_module_is_untouched_by_the_logger() -> None:
    """The logger writes to stderr only: stdout is the protocol channel."""
    from twitch_radio.extractor_worker import _StderrLogger

    sink_out, sink_err = io.StringIO(), io.StringIO()
    real_out, real_err = sys.stdout, sys.stderr
    sys.stdout, sys.stderr = sink_out, sink_err
    try:
        logger = _StderrLogger(verbose=False)
        logger.debug("quiet")
        logger.warning("loud\nsecond line")
        logger.error("bad")
    finally:
        sys.stdout, sys.stderr = real_out, real_err
    assert sink_out.getvalue() == ""
    assert sink_err.getvalue() == "WARNING: loud\nWARNING: second line\nERROR: bad\n"
