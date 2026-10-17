// init(): how the window starts listening to the shell. The shell is replaced by a stand-in that behaves
// like Tauri in the one way that matters here: an event emitted while nobody listens is gone for good.
import { beforeEach, describe, expect, it, vi } from 'vitest';

const shell = vi.hoisted(() => {
  const listeners = new Map<string, Set<(payload: unknown) => void>>();
  const listen = (event: string) =>
    vi.fn(async (callback: (payload: unknown) => void) => {
      const set = listeners.get(event) ?? new Set();
      listeners.set(event, set);
      set.add(callback);
      return () => set.delete(callback);
    });
  const api = {
    appInfo: vi.fn(), getPrefs: vi.fn(), getStatus: vi.fn(), getLogs: vi.fn(), preflight: vi.fn(),
    lastAppUpdate: vi.fn(), setAppearance: vi.fn(),
    onStatus: listen('bot:status'), onState: listen('bot:state'), onLogs: listen('logs:batch'),
    onPrefs: listen('prefs:changed'), onAppUpdate: listen('update:app'), onYtdlpUpdate: listen('update:ytdlp'),
    onCloseRequested: listen('close:requested'),
  };
  /** What Tauri does: delivers to whoever is listening right now, to nobody otherwise. */
  const emit = (event: string, payload: unknown) => listeners.get(event)?.forEach((cb) => cb(payload));
  return { api, listeners, emit };
});
vi.mock('../lib/api', () => ({ api: shell.api }));

import { logs } from '../lib/logs.svelte';
import { init, toasts } from '../lib/store.svelte';
import type { LogEntry, Snapshot } from '../lib/types';

const line = (id: number): LogEntry => ({ id, ts: id, level: 'INFO', logger: 'core', msg: `line ${id}`, cont: false });
const status = { phase: 'running', pid: 1, fatal: null, exitCode: null, restartAt: null, startedAt: 1, home: '/h' };
const ids = () => logs.all.map((e) => e.id);

beforeEach(() => {
  // jsdom has no matchMedia; init() watches the system colour scheme.
  window.matchMedia = vi.fn().mockReturnValue({
    matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn(),
  }) as unknown as typeof window.matchMedia;
  shell.listeners.clear();
  for (const fn of Object.values(shell.api)) fn.mockClear();
  shell.api.appInfo.mockResolvedValue({ version: '1', home: '/h', packaged: false, platform: 'linux', runtime: 'tauri' });
  shell.api.getPrefs.mockResolvedValue({ theme: 'dark', windowBg: '#000', windowDark: true });
  shell.api.getStatus.mockResolvedValue({ status, state: null });
  shell.api.preflight.mockResolvedValue({});
  shell.api.setAppearance.mockResolvedValue(undefined);
  shell.api.lastAppUpdate.mockResolvedValue(null);
  shell.api.getLogs.mockResolvedValue([line(1), line(2)]);
  toasts.splice(0);
});

describe('init', () => {
  it('a line the shell sends while the snapshot is being fetched is not lost', async () => {
    // The old order (snapshot, then listen) lost line 3: it is newer than the snapshot and its one batch
    // went out while nobody was listening.
    let release: (entries: LogEntry[]) => void = () => {};
    shell.api.getLogs.mockReturnValue(new Promise<LogEntry[]>((resolve) => (release = resolve)));
    const starting = init();
    await vi.waitFor(() => expect(shell.api.getLogs).toHaveBeenCalled());
    shell.emit('logs:batch', [line(3)]); // arrives before the snapshot's answer
    release([line(1), line(2)]);
    const stop = await starting;
    shell.emit('logs:batch', [line(4)]);
    expect(ids()).toEqual([1, 2, 3, 4]);
    stop();
  });

  it('a line that is in both the snapshot and an early batch is shown once', async () => {
    let release: (entries: LogEntry[]) => void = () => {};
    shell.api.getLogs.mockReturnValue(new Promise<LogEntry[]>((resolve) => (release = resolve)));
    const starting = init();
    await vi.waitFor(() => expect(shell.api.getLogs).toHaveBeenCalled());
    shell.emit('logs:batch', [line(2), line(3)]);
    release([line(1), line(2)]);
    const stop = await starting;
    expect(ids()).toEqual([1, 2, 3]);
    stop();
  });

  it('a failed start leaves no listeners behind and says what happened on the console', async () => {
    const error = vi.spyOn(console, 'error').mockImplementation(() => {});
    shell.api.onPrefs.mockRejectedValueOnce(new Error('listen refused'));
    await expect(init()).rejects.toThrow('listen refused');
    const remaining = [...shell.listeners.values()].reduce((n, set) => n + set.size, 0);
    expect(remaining).toBe(0); // the ones registered before the failure were stopped too
    expect(error).toHaveBeenCalled();
    error.mockRestore();
  });

  it('a song that fails to load is reported once, and one that failed before the window opened is not', async () => {
    const old: Partial<Snapshot> = { last_failure: { at: 100, message: 'Old failure' } };
    shell.api.getStatus.mockResolvedValue({ status, state: old });
    const stop = await init();
    shell.emit('bot:state', old);
    expect(toasts).toHaveLength(0);

    const fresh = { last_failure: { at: 200, message: "Couldn't load Streamer's song \u2014 skipping it." } };
    shell.emit('bot:state', fresh);
    shell.emit('bot:state', fresh); // the next report repeats it
    expect(toasts.map((t) => [t.message, t.kind])).toEqual([[fresh.last_failure.message, 'err']]);
    stop();
  });
});
