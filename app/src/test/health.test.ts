import { describe, expect, it } from 'vitest';
import { healthItems, queueSummary, setupSteps, tokensOf } from '../lib/health';
import type { Preflight, Snapshot } from '../lib/types';

const snap = (over: Partial<Snapshot> = {}): Snapshot => ({
  uptime: 10,
  now: null,
  queue: [],
  queue_size: 0,
  counters: { requests_queued: 0, tracks_played: 0, skips: 0, tracks_failed: 0, resolve_success: 0, resolve_failure: 0 },
  player: { paused: false, listeners: 0, encoder_running: false, encoder_starts: 0 },
  chat: { ready: true, subscribed: true, tokens: { bot: true, owner: true } },
  http: { up: true, base: 'http://127.0.0.1:8098', stream: 's', overlay: 'o', settings: 'p' },
  workers: { alive: 0, size: 2 },
  health: { loop_lag_ms: 3 },
  ...over,
});
const pre = (over: Partial<Preflight> = {}): Preflight => ({
  ok: true,
  config_ok: true,
  tokens: { bot: true, owner: true },
  ffmpeg: { path: '/usr/bin/ffmpeg' },
  js_runtime: { name: 'quickjs', path: '/x/qjs' },
  js_solver: { ready: false },
  ...over,
});
const find = (items: ReturnType<typeof healthItems>, name: string) => items.find((i) => i.name.startsWith(name));

describe('health', () => {
  it('offline: everything about the running bot is idle, the tool checks still show', () => {
    const items = healthItems({ live: false, snap: null, pre: pre() });
    expect(find(items, 'Twitch chat')).toMatchObject({ state: 'idle', value: 'Offline' });
    expect(find(items, 'Audio encoder')?.state).toBe('idle');
    expect(find(items, 'ffmpeg')).toMatchObject({ state: 'ok', value: 'Found' });
    expect(find(items, 'JS runtime (quickjs)')?.state).toBe('ok');
    expect(find(items, 'Event-loop delay')).toBeUndefined();
  });

  it('running: chat, web server and the encoder report what the bot says', () => {
    const items = healthItems({ live: true, snap: snap({ player: { paused: false, listeners: 1, encoder_running: true, encoder_starts: 3 } }), pre: pre() });
    expect(find(items, 'Twitch chat')).toMatchObject({ state: 'ok', value: 'Connected' });
    expect(find(items, 'Web server')?.value).toContain('127.0.0.1:8098');
    expect(find(items, 'Audio encoder')).toMatchObject({ state: 'warn', value: 'Restarted 2×' });
    expect(find(items, 'Event-loop delay')).toMatchObject({ state: 'ok', value: '3 ms' });
  });

  it('missing ffmpeg is an error and a slow event loop a warning', () => {
    const items = healthItems({ live: true, snap: snap({ health: { loop_lag_ms: 120 } }), pre: pre({ ffmpeg: { path: null } }) });
    expect(find(items, 'ffmpeg')).toMatchObject({ state: 'err', value: 'Not found' });
    expect(find(items, 'Event-loop delay')?.state).toBe('warn');
  });

  it('an unreadable token file is an error on both sign-ins', () => {
    const items = healthItems({ live: false, snap: null, pre: pre({ tokens: { readable: false } }) });
    expect(find(items, 'Bot account')?.state).toBe('err');
    expect(find(items, 'Broadcaster')?.state).toBe('err');
  });

  it('tokens come from the live report when there is one, else from the preflight', () => {
    expect(tokensOf({ snap: snap({ chat: { ready: true, subscribed: true, tokens: { bot: true } } }), pre: pre({ tokens: { owner: true } }) })).toEqual({ bot: true });
    expect(tokensOf({ snap: null, pre: pre({ tokens: { owner: true } }) })).toEqual({ owner: true });
    expect(tokensOf({ snap: null, pre: null })).toEqual({});
  });

  it('setup: credentials first, then the two authorizations', () => {
    const steps = setupSteps({ live: false, snap: null, pre: pre({ config_ok: false, tokens: {} }) });
    expect(steps.map((s) => [s.id, s.done])).toEqual([['credentials', false], ['bot', false], ['owner', false]]);
    expect(steps[1]?.desc).toContain('Press Start');
    const done = setupSteps({ live: true, snap: null, pre: pre() });
    expect(done.every((s) => s.done)).toBe(true);
  });

  it('queue summary: requested versus radio, pause and auto-radio off', () => {
    expect(queueSummary(snap({ queue_size: 7, queue_requests: 2 }))).toBe('2 requested · 5 from radio');
    expect(queueSummary(snap({ queue_size: 3, queue_requests: 3, radio_autoplay: false }))).toBe('3 requested · auto-radio off');
    expect(queueSummary(snap({ player: { paused: true, listeners: 0, encoder_running: false, encoder_starts: 0 } }))).toBe('Playback paused');
  });
});
