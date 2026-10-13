// The window's pieces, mounted for real (jsdom) with the shell replaced by a stand-in.
import { flushSync, mount, tick, unmount, type Component } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const api = vi.hoisted(() => {
  const fn = () => vi.fn();
  return {
    getStatus: fn(), start: fn(), stop: fn(), restart: fn(), command: fn(), requestSong: fn(), searchSongs: fn(),
    getLookahead: fn(), setLookahead: fn(), getLogs: fn(), clearLogs: fn(), exportLogs: fn(), preflight: fn(),
    getEnv: fn(), saveEnv: fn(), getLive: fn(), saveLive: fn(), getPrefs: fn(), setPrefs: fn(), setAppearance: fn(),
    uiReady: fn(), hideToTray: fn(), closeDecision: fn(), checkAppUpdate: fn(), lastAppUpdate: fn(), ytdlpCheck: fn(),
    ytdlpInstall: fn(), ytdlpRollback: fn(), openExternal: fn(), openPath: fn(), importCookies: fn(), appInfo: fn(),
  };
});
vi.mock('../lib/api', () => ({ api }));

import CloseDialog from '../components/CloseDialog.svelte';
import Dashboard from '../components/dashboard/Dashboard.svelte';
import LogsTab from '../components/LogsTab.svelte';
import Sidebar from '../components/Sidebar.svelte';
import SettingsTab from '../components/SettingsTab.svelte';
import { discard, form } from '../lib/form.svelte';
import { LOG_CAP, addLogs, logView, logs, setLogs } from '../lib/logs.svelte';
import { bot, info, prefs, toasts, ui } from '../lib/store.svelte';
import { applyAppearance, appearance } from '../lib/theme.svelte';
import type { Snapshot } from '../lib/types';

let target: HTMLElement;
let app: ReturnType<typeof mount> | undefined;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const show = (component: Component<any>, props: Record<string, unknown> = {}) => {
  app = mount(component, { target, props });
  flushSync();
};
const settle = async () => {
  for (let i = 0; i < 5; i++) await tick();
};

const snapshot = (over: Partial<Snapshot> = {}): Snapshot => ({
  uptime: 5,
  now: { title: 'Song A', uploader: 'Band', requester: 'viewer', elapsed: 10, duration: 100 },
  queue: [{ title: 'Song B', requester: 'viewer' }, { title: 'Mix 1', requester: '', radio: true }],
  queue_size: 2,
  queue_requests: 1,
  counters: { requests_queued: 1, tracks_played: 2, skips: 0, tracks_failed: 0, resolve_success: 1, resolve_failure: 0 },
  player: { paused: false, listeners: 1, encoder_running: true, encoder_starts: 1, next_ready: true },
  chat: { ready: true, subscribed: true, tokens: { bot: true, owner: true } },
  http: { up: true, base: 'http://127.0.0.1:8098', stream: 'http://localhost:8098/stream.opus', overlay: 'http://localhost:8098/overlay', settings: 'http://localhost:8098/settings' },
  workers: { alive: 0, size: 2 },
  health: { loop_lag_ms: 2 },
  ...over,
});
const goLive = (snap: Snapshot | null = snapshot()) => {
  bot.status = { ...bot.status, phase: 'running', startedAt: Date.now() };
  bot.snap = snap;
  bot.snapAt = Date.now();
  bot.pre = { ok: true, config_ok: true, tokens: { bot: true, owner: true }, ffmpeg: { path: '/f' }, js_runtime: { name: 'quickjs', path: '/q' } };
};
const click = (el: Element | null | undefined) => {
  expect(el, 'element to click').toBeTruthy();
  (el as HTMLElement).click();
};
const button = (label: string) => [...target.querySelectorAll('button')].find((b) => b.textContent?.trim() === label);

beforeEach(() => {
  target = document.createElement('div');
  document.body.append(target);
  for (const fn of Object.values(api)) fn.mockReset();
  api.getLookahead.mockResolvedValue({ enabled: true, count: 5 });
  api.setLookahead.mockImplementation(async (v) => v);
  api.setPrefs.mockImplementation(async (p) => ({ ...prefs, ...p }));
  api.setAppearance.mockResolvedValue(undefined);
  bot.status = { phase: 'stopped', pid: null, fatal: null, exitCode: null, restartAt: null, startedAt: null, home: '' };
  bot.snap = null;
  bot.pre = null;
  ui.tab = 'dashboard';
  ui.closeAsk = false;
  toasts.length = 0;
  Object.assign(info, { version: '1.3.0', home: '/data', packaged: true, platform: 'linux' });
});
afterEach(() => {
  if (app) void unmount(app);
  app = undefined;
  target.remove();
});

describe('dashboard', () => {
  it('stopped: nothing playing, the request bar is disabled with a reason', () => {
    show(Dashboard);
    expect(target.textContent).toContain('Nothing playing');
    const input = target.querySelector<HTMLInputElement>('input[type="search"]')!;
    expect(input.disabled).toBe(true);
    expect(input.title).toBe('Start the bot first');
  });

  it('running: shows the song, the queue, and where the songs came from', () => {
    goLive();
    show(Dashboard);
    expect(target.querySelector('.now-title')?.textContent).toBe('Song A');
    expect([...target.querySelectorAll('.qlist .q-title')].map((n) => n.textContent)).toEqual(['Song B', 'Mix 1']);
    expect(target.textContent).toContain('1 requested · 1 from radio');
    expect(target.textContent).toContain('Requested by viewer');
    expect(target.querySelector<HTMLElement>('.bar')?.style.width).toMatch(/^(1[0-9]|10)(\.\d)?%$/);
  });

  it('skip: a refused skip tells the person why, and the button says the next song is loading', async () => {
    goLive(snapshot({ player: { paused: false, listeners: 1, encoder_running: true, encoder_starts: 1, next_ready: false } }));
    api.command.mockResolvedValue({ ok: false, error: 'The next song is still loading, so the current one keeps playing. Try again in a moment.' });
    show(Dashboard);
    const skip = button('Skip')!;
    expect(skip.title).toContain('still loading');
    click(skip);
    await settle();
    expect(api.command).toHaveBeenCalledWith('skip');
    expect(toasts.at(-1)).toMatchObject({ kind: 'err' });
    expect(toasts.at(-1)?.message).toContain('still loading');
  });

  it('paused: says so, says what Resume does, and Resume is the main button', async () => {
    goLive(snapshot({ now: null, player: { paused: true, listeners: 1, encoder_running: true, encoder_starts: 1 } }));
    api.command.mockResolvedValue({ ok: true });
    show(Dashboard);
    expect(target.querySelector('.now-title')?.textContent).toBe('Paused');
    expect(target.textContent).toContain('Resume starts “Song B” again from the beginning.');
    expect(target.textContent).not.toContain('Waiting for a request');
    expect(target.querySelector('.tag.warn')?.textContent).toBe('Paused');
    const resume = button('Resume')!;
    expect(resume.classList.contains('primary')).toBe(true);
    expect(button('Skip')!.disabled).toBe(true); // nothing is playing to skip
    click(resume);
    await settle();
    expect(api.command).toHaveBeenCalledWith('resume');
  });

  it('skip: when the next song is ready there is no hint', () => {
    goLive();
    show(Dashboard);
    expect(button('Skip')!.title).toBe('');
  });

  it('a typed search lists matches; choosing one queues it', async () => {
    goLive();
    api.searchSongs.mockResolvedValue({ ok: true, results: [{ title: 'Found', url: 'https://youtu.be/x', uploader: 'Chan', duration: 125 }] });
    api.requestSong.mockResolvedValue({ ok: true, title: 'Found', position: 3 });
    show(Dashboard);
    const input = target.querySelector<HTMLInputElement>('input[type="search"]')!;
    input.value = 'daft punk';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    expect(button('Search')).toBeTruthy();
    target.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    await settle();
    expect(api.searchSongs).toHaveBeenCalledWith('daft punk');
    expect(target.querySelector('.r-title')?.textContent).toBe('Found');
    expect(target.querySelector('.r-by')?.textContent).toBe('Chan · 2:05');
    click(target.querySelector('.result'));
    await settle();
    expect(api.requestSong).toHaveBeenCalledWith('https://youtu.be/x');
    expect(target.querySelector('.search-status')?.textContent).toContain('Added: Found (#3 in the queue)');
  });

  it('a pasted link is queued straight away', async () => {
    goLive();
    api.requestSong.mockResolvedValue({ ok: true, title: 'Direct', position: 1 });
    show(Dashboard);
    const input = target.querySelector<HTMLInputElement>('input[type="search"]')!;
    input.value = 'https://youtu.be/abc';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    expect(button('Add to queue')).toBeTruthy();
    target.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    await settle();
    expect(api.searchSongs).not.toHaveBeenCalled();
    expect(api.requestSong).toHaveBeenCalledWith('https://youtu.be/abc');
  });

  it('clearing the queue takes two clicks', async () => {
    goLive();
    api.command.mockResolvedValue({ ok: true });
    show(Dashboard);
    click(button('Clear'));
    flushSync();
    expect(api.command).not.toHaveBeenCalled();
    click(button('Click again to clear'));
    await settle();
    expect(api.command).toHaveBeenCalledWith('clear_queue');
  });

  it('the setup card appears until the credentials and both sign-ins are done', () => {
    bot.pre = { config_ok: false, tokens: {}, ffmpeg: { path: '/f' } };
    show(Dashboard);
    expect(target.textContent).toContain('Finish setting up');
    expect(target.textContent).toContain('0 of 3 done');
  });

  it('setup: Authorize waits for the running bot, and every step can copy its link', async () => {
    bot.pre = { config_ok: true, tokens: {}, ffmpeg: { path: '/f' } };
    show(Dashboard);
    const authorize = [...target.querySelectorAll<HTMLButtonElement>('.steps button')].filter((b) => b.textContent === 'Authorize');
    expect(authorize).toHaveLength(2);
    expect(authorize.every((b) => b.disabled && b.title === 'Start the bot first')).toBe(true);
    const copyLinks = [...target.querySelectorAll('.steps button')].filter((b) => b.textContent === 'Copy link');
    expect(copyLinks).toHaveLength(2);
    Object.assign(navigator, { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } });
    click(copyLinks[0]);
    await settle();
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(expect.stringContaining('scopes=user:read:chat'));
    goLive(snapshot({ chat: { ready: true, subscribed: false, tokens: {} } }));
    bot.pre = { config_ok: true, tokens: {}, ffmpeg: { path: '/f' } };
    await settle();
    const live = [...target.querySelectorAll<HTMLButtonElement>('.steps button')].filter((b) => b.textContent === 'Authorize');
    expect(live.every((b) => !b.disabled)).toBe(true);
    click(live[0]);
    expect(api.openExternal).toHaveBeenCalledWith(expect.stringContaining('/oauth?scopes='));
  });

  it('setup: the credentials step offers the redirect URL to copy', () => {
    bot.pre = { config_ok: false, tokens: {} };
    show(Dashboard);
    expect(button('Copy redirect URL')).toBeTruthy();
    expect(button('Open Settings')).toBeTruthy();
  });

  it('a failed start shows what went wrong and where to look', () => {
    bot.status = { ...bot.status, phase: 'error', fatal: { message: 'Port 8098 is in use.', hint: 'Change the port in Settings.', code: 2 } };
    show(Dashboard);
    expect(target.querySelector('.banner.err')?.textContent).toContain('Port 8098 is in use.');
  });
});

describe('close dialog', () => {
  it('keeping it running tells the shell, and "don\'t ask again" is passed along', async () => {
    ui.closeAsk = true;
    show(CloseDialog);
    expect(document.activeElement?.textContent).toBe('Keep running in tray');
    click(target.querySelector('input[type="checkbox"]'));
    click(button('Keep running in tray'));
    await settle();
    expect(api.closeDecision).toHaveBeenCalledWith('tray', true);
    expect(ui.closeAsk).toBe(false);
  });

  it('cancel and Escape never remember anything', async () => {
    ui.closeAsk = true;
    show(CloseDialog);
    click(target.querySelector('input[type="checkbox"]'));
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await settle();
    expect(api.closeDecision).toHaveBeenCalledWith('cancel', false);
  });
});

describe('sidebar', () => {
  it('shows unseen errors on the Logs entry until the tab is opened', () => {
    logView.unseenErrors = 120;
    show(Sidebar);
    expect(target.querySelector('.badge')?.textContent).toBe('99+');
    click([...target.querySelectorAll('.nav')].find((n) => n.textContent?.includes('Logs')));
    flushSync();
    expect(ui.tab).toBe('logs');
    expect(target.querySelector('.badge')).toBeNull();
    logView.unseenErrors = 0;
  });
});

describe('logs', () => {
  const entries = (n: number) => Array.from({ length: n }, (_, i) => ({ id: i + 1, ts: 1_760_000_000 + i, level: i % 50 === 0 ? 'ERROR' : 'INFO', logger: 'core', msg: `line ${i}`, cont: false }));

  it('only the rows in view are in the page, however many lines there are', async () => {
    setLogs(entries(LOG_CAP));
    ui.tab = 'logs';
    show(LogsTab);
    await settle();
    expect(logs.all.length).toBe(LOG_CAP);
    const rendered = target.querySelectorAll('.ll').length;
    expect(rendered).toBeGreaterThan(0);
    expect(rendered).toBeLessThan(120);
    expect(target.querySelector('.log-foot')?.textContent).toContain('5,000 of 5,000 lines');
  });

  it('lines that arrive while the tab is open appear without reopening it', async () => {
    setLogs(entries(3));
    show(LogsTab);
    await settle();
    expect(target.querySelectorAll('.ll').length).toBe(3);
    addLogs(entries(8).slice(3).map((e) => ({ ...e, id: e.id + 100 })), true);
    await settle();
    expect(target.querySelectorAll('.ll').length).toBe(8);
    expect(target.querySelector('.log-foot')?.textContent).toContain('8 of 8 lines');
  });

  it('level chips and the search box narrow the view', async () => {
    setLogs(entries(200));
    show(LogsTab);
    await settle();
    click([...target.querySelectorAll('.chip')].find((c) => c.textContent === 'Info'));
    await settle();
    expect(target.querySelector('.log-foot')?.textContent).toContain('4 of 200 lines'); // the four ERROR lines
    click([...target.querySelectorAll('.chip')].find((c) => c.textContent === 'Info'));
    logView.query = 'line 19';
    await settle();
    expect(target.querySelector('.log-foot')?.textContent).toMatch(/^11 of 200/);
    logView.query = '';
  });
});

describe('appearance', () => {
  it('applies light/dark and the colour style to the page and tells the shell the background', () => {
    appearance.systemDark = true;
    const root = document.createElement('html');
    applyAppearance({ theme: 'system', skin: 'midnight' }, root);
    expect(root.dataset.theme).toBe('dark');
    expect(root.style.getPropertyValue('--accent')).toBe('#4f8cff');
    applyAppearance({ theme: 'light', skin: 'default' }, root);
    expect(root.dataset.theme).toBe('light');
    expect(root.style.getPropertyValue('--accent')).toBe(''); // the default style removes the overrides
    expect(api.setAppearance).toHaveBeenCalled();
    const [bg, dark] = api.setAppearance.mock.calls.at(-1)!;
    expect(bg).toMatch(/^#[0-9a-f]{6}$/);
    expect(dark).toBe(false);
  });
});

describe('settings', () => {
  const env = { values: { TWITCH_CLIENT_ID: 'abc', LOG_LEVEL: 'info', LOG_TO_FILE: '' }, secretSet: true, envPath: '/data/.env' };
  beforeEach(() => {
    discard();
    form.loaded = false;
    form.values = {};
    form.orig = {};
    api.getEnv.mockResolvedValue(env);
    api.getLive.mockResolvedValue({ tunables: { queue_cap: 50 }, bounds: { queue_cap: [1, 500] }, radio_autoplay_enabled: true });
    api.saveEnv.mockResolvedValue({ ok: true, preflight: { ok: true, config_ok: true }, needsRestart: true });
  });

  it('loads the values, shows the secret as set, and offers Save only after a change', async () => {
    show(SettingsTab);
    await settle();
    expect(target.querySelector<HTMLInputElement>('#f-TWITCH_CLIENT_ID')?.value).toBe('abc');
    expect(target.querySelector<HTMLSelectElement>('#f-LOG_LEVEL')?.value).toBe('INFO');
    expect(target.querySelector('.secret .tag')?.textContent).toBe('set');
    expect(target.querySelector('.savebar')).toBeNull();
    const id = target.querySelector<HTMLInputElement>('#f-TWITCH_CLIENT_ID')!;
    id.value = 'changed';
    id.dispatchEvent(new Event('input', { bubbles: true }));
    await settle();
    expect(target.querySelector('.savebar')).toBeTruthy();
  });

  it('saves only what changed, then asks for a restart', async () => {
    show(SettingsTab);
    await settle();
    const id = target.querySelector<HTMLInputElement>('#f-TWITCH_CLIENT_ID')!;
    id.value = 'changed';
    id.dispatchEvent(new Event('input', { bubbles: true }));
    await settle();
    click(button('Save changes'));
    await settle();
    expect(api.saveEnv).toHaveBeenCalledWith({ values: { TWITCH_CLIENT_ID: 'changed' } });
    expect(target.textContent).toContain('Restart the bot for these changes to take effect');
  });

  it('a bad number is flagged and nothing is sent', async () => {
    show(SettingsTab);
    await settle();
    const port = target.querySelector<HTMLInputElement>('#f-TWITCH_NOWPLAYING_PORT')!;
    port.value = '80';
    port.dispatchEvent(new Event('input', { bubbles: true }));
    await settle();
    click(button('Save changes'));
    await settle();
    expect(api.saveEnv).not.toHaveBeenCalled();
    expect(target.querySelector('.bad-msg')?.textContent).toContain('whole number from 1024 to 65535');
    expect(target.querySelector('.savebar')?.textContent).toContain('Fix the highlighted fields first.');
  });
});
