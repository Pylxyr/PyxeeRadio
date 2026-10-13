// What the whole window shares: the bot's status and last report, the setup check, the window
// preferences, notices. Components read these objects and call the actions below; nothing here draws.
import { api } from './api';
import { applyAppearance, watchSystemTheme } from './theme.svelte';
import { addLogs, resetUnseen, setLogs } from './logs.svelte';
import type { AppInfo, AppUpdate, BotStatus, Preflight, Prefs, Snapshot, YtdlpReport } from './types';

export type Tab = 'dashboard' | 'logs' | 'settings';

export const DEFAULT_PREFS: Prefs = {
  askOnClose: true,
  closeAction: 'tray',
  autoRestart: true,
  startBotOnLaunch: false,
  theme: 'system',
  skin: 'default',
  launchAtLogin: false,
  checkAppUpdates: true,
  checkYtdlpOnStartup: true,
  lastUpdateCheck: 0,
  lastYtdlpCheck: 0,
  windowBg: '#0b0b10',
  windowDark: true,
};

export const bot = $state<{ status: BotStatus; snap: Snapshot | null; snapAt: number; pre: Preflight | null }>({
  status: { phase: 'stopped', pid: null, fatal: null, exitCode: null, restartAt: null, startedAt: null, home: '' },
  snap: null,
  snapAt: 0,
  pre: null,
});
export const prefs = $state<Prefs>({ ...DEFAULT_PREFS });
export const info = $state<AppInfo>({ version: '', home: '', packaged: false, platform: 'linux', runtime: 'tauri' });
export const ui = $state<{ tab: Tab; ready: boolean; closeAsk: boolean }>({ tab: 'dashboard', ready: false, closeAsk: false });
export const upd = $state<{ app: AppUpdate | null; ytdlp: YtdlpReport | null; busy: '' | 'app' | 'check' | 'install' | 'rollback' }>({
  app: null,
  ytdlp: null,
  busy: '',
});
/** Ticks once a second while the window is on screen: the progress bar, uptime and restart countdown read it. */
export const clock = $state({ now: Date.now() });

export const isLive = () => bot.status.phase === 'running' || bot.status.phase === 'starting';

// ----------------------------------------------------------------- notices ---
export interface Toast {
  id: number;
  message: string;
  kind: '' | 'ok' | 'err';
}
export const toasts = $state<Toast[]>([]);
let toastId = 0;
export function toast(message: string, kind: Toast['kind'] = ''): void {
  const id = ++toastId;
  toasts.push({ id, message, kind });
  setTimeout(() => {
    const at = toasts.findIndex((t) => t.id === id);
    if (at !== -1) toasts.splice(at, 1);
  }, kind === 'err' ? 7000 : 3500);
}
export async function copyText(text: string, what = 'Copied'): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    toast(`${what} to clipboard.`, 'ok');
  } catch {
    toast("Couldn't access the clipboard.", 'err');
  }
}

// ----------------------------------------------------------------- actions ---
export function goto(tab: Tab): void {
  ui.tab = tab;
  if (tab === 'logs') resetUnseen();
}

export function applyPrefs(next: Prefs): void {
  Object.assign(prefs, next);
  applyAppearance(prefs);
}

export async function setPrefs(patch: Partial<Prefs>): Promise<void> {
  applyPrefs(await api.setPrefs(patch));
}

export async function refreshPreflight(force = true): Promise<void> {
  try {
    bot.pre = await api.preflight(force);
  } catch {
    /* the checklist just stays in its "checking" state */
  }
}

export async function command(name: 'skip' | 'pause' | 'resume' | 'clear_queue', okMessage?: string): Promise<void> {
  const result = await api.command(name);
  if (!result.ok) toast(result.error || (name === 'skip' ? 'Nothing to skip.' : 'That did nothing.'), 'err');
  else if (okMessage) toast(okMessage, 'ok');
}

export async function startBot(): Promise<void> {
  bot.status = await api.start();
}

let stopWatching: (() => void) | null = null;
const unlisten: (() => void)[] = [];

/** Loads everything the window needs and starts listening to the shell. Returns a function that stops it. */
export async function init(): Promise<() => void> {
  stopWatching = watchSystemTheme(() => applyAppearance(prefs));
  const [appInfo, savedPrefs] = await Promise.all([api.appInfo(), api.getPrefs()]);
  Object.assign(info, appInfo);
  applyPrefs(savedPrefs);
  const current = await api.getStatus();
  bot.status = current.status;
  bot.snap = current.state;
  bot.snapAt = Date.now();
  setLogs(await api.getLogs());
  resetUnseen(); // history from before the window opened isn't news

  let lastPhase = bot.status.phase;
  unlisten.push(
    await api.onStatus((next) => {
      bot.status = next;
      if (next.phase === 'stopped' || next.phase === 'error') bot.snap = null;
      if (next.phase !== lastPhase && ['stopped', 'running', 'error'].includes(next.phase)) void refreshPreflight(true);
      lastPhase = next.phase;
    }),
    await api.onState((next) => {
      bot.snap = next;
      bot.snapAt = Date.now();
    }),
    await api.onLogs((batch) => addLogs(batch, ui.tab === 'logs')),
    await api.onPrefs(applyPrefs),
    await api.onAppUpdate((result) => (upd.app = result)),
    await api.onYtdlpUpdate((result) => (upd.ytdlp = result)),
    await api.onCloseRequested(() => (ui.closeAsk = true)),
  );
  api.lastAppUpdate().then((result) => {
    if (result) upd.app = result;
  }).catch(() => {});

  // Nothing polls. The report is refreshed when something changed (start/stop, a save) and when the
  // window regains focus (say, after installing ffmpeg); the shell answers from its cache when the
  // last check is under two minutes old, so this costs one short check per two minutes of active use.
  void refreshPreflight(false);
  const onFocus = () => {
    if (!isLive()) void refreshPreflight(false);
  };
  window.addEventListener('focus', onFocus);
  const timer = setInterval(() => {
    if (!document.hidden) clock.now = Date.now();
  }, 1000);
  ui.ready = true;
  return () => {
    clearInterval(timer);
    window.removeEventListener('focus', onFocus);
    stopWatching?.();
    for (const stop of unlisten.splice(0)) stop();
  };
}
