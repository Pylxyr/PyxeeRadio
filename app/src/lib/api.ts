// The only bridge between the page and the shell: named functions, one per command in
// src-tauri/src/commands.rs. The page gets nothing else (see capabilities/default.json).
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  Ack, AppInfo, AppUpdate, BotStatus, EnvData, LiveData, Lookahead, LogEntry, Preflight, Prefs,
  SaveEnvResult, SearchResult, Snapshot, YtdlpReport,
} from './types';

type Unlisten = () => void;
const subscribe =
  <T>(event: string) =>
  async (callback: (payload: T) => void): Promise<Unlisten> =>
    listen<T>(event, (e) => callback(e.payload));

export const api = {
  // the bot
  getStatus: () => invoke<{ status: BotStatus; state: Snapshot | null }>('bot_status'),
  start: () => invoke<BotStatus>('bot_start'),
  stop: () => invoke<BotStatus>('bot_stop'),
  restart: () => invoke<BotStatus>('bot_restart'),
  command: (name: 'skip' | 'pause' | 'resume' | 'clear_queue') => invoke<Ack>('bot_command', { name }),
  requestSong: (text: string) => invoke<Ack & { title?: string; position?: number }>('bot_request_song', { text }),
  searchSongs: (text: string) => invoke<Ack & { results: SearchResult[] }>('bot_search_songs', { text }),
  getLookahead: () => invoke<Lookahead>('radio_get_lookahead'),
  setLookahead: (value: Lookahead) => invoke<Lookahead>('radio_set_lookahead', { value }),
  onStatus: subscribe<BotStatus>('bot:status'),
  onState: subscribe<Snapshot | null>('bot:state'),
  // logs
  getLogs: () => invoke<LogEntry[]>('logs_get'),
  clearLogs: () => invoke<boolean>('logs_clear'),
  exportLogs: () => invoke<{ ok: boolean; canceled?: boolean; error?: string }>('logs_export'),
  onLogs: subscribe<LogEntry[]>('logs:batch'),
  // configuration
  preflight: (force = true) => invoke<Preflight>('config_preflight', { force }),
  getEnv: () => invoke<EnvData>('config_get_env'),
  saveEnv: (payload: { values: Record<string, string> }) => invoke<SaveEnvResult>('config_save_env', { payload }),
  getLive: () => invoke<LiveData>('config_get_live'),
  saveLive: (payload: { tunables: Record<string, string | number>; radio_autoplay_enabled: boolean }) =>
    invoke<LiveData>('config_save_live', { payload }),
  getPrefs: () => invoke<Prefs>('prefs_get'),
  setPrefs: (patch: Partial<Prefs>) => invoke<Prefs>('prefs_set', { patch }),
  onPrefs: subscribe<Prefs>('prefs:changed'),
  setAppearance: (bg: string, dark: boolean) => invoke<void>('set_appearance', { bg, dark }),
  // the window
  uiReady: () => invoke<void>('ui_ready'),
  hideToTray: () => invoke<void>('hide_to_tray'),
  closeDecision: (action: 'tray' | 'quit' | 'cancel', remember: boolean) => invoke<void>('close_decision', { action, remember }),
  onCloseRequested: subscribe<null>('close:requested'),
  // updates
  checkAppUpdate: () => invoke<AppUpdate>('update_app'),
  lastAppUpdate: () => invoke<AppUpdate | null>('update_app_last'),
  ytdlpCheck: () => invoke<YtdlpReport>('update_ytdlp_check'),
  ytdlpInstall: () => invoke<YtdlpReport>('update_ytdlp_install'),
  ytdlpRollback: () => invoke<YtdlpReport>('update_ytdlp_rollback'),
  onAppUpdate: subscribe<AppUpdate>('update:app'),
  onYtdlpUpdate: subscribe<YtdlpReport>('update:ytdlp'),
  // shell helpers
  openExternal: (url: string) => invoke<boolean>('open_external', { url }),
  openPath: (which: 'home' | 'data' | 'logs' | 'env') => invoke<boolean>('open_path', { which }),
  importCookies: () => invoke<string | null>('import_cookies'),
  appInfo: () => invoke<AppInfo>('app_info'),
};

export type Api = typeof api;
