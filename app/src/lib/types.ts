// The shapes of what the shell and the bot send the page. Only what the page reads is listed.

export type Phase = 'stopped' | 'starting' | 'running' | 'stopping' | 'error';

export interface Fatal {
  message: string;
  hint: string;
  code: number;
}

export interface BotStatus {
  phase: Phase;
  pid: number | null;
  fatal: Fatal | null;
  exitCode: number | null;
  restartAt: number | null;
  startedAt: number | null;
  home: string;
}

export interface NowPlaying {
  title: string;
  uploader?: string;
  requester: string;
  radio?: boolean;
  thumbnail?: string;
  elapsed: number;
  duration: number;
}

export interface QueueItem {
  title: string;
  requester: string;
  radio?: boolean;
  thumbnail?: string;
  /** Seconds into the song it will carry on from: set for a song that was paused. */
  resume_at?: number;
}

export interface Counters {
  requests_queued: number;
  tracks_played: number;
  skips: number;
  tracks_failed: number;
  resolve_success: number;
  resolve_failure: number;
}

export interface Lookahead {
  enabled: boolean;
  count: number;
}

export interface Tokens {
  bot?: boolean;
  owner?: boolean;
  readable?: boolean;
}

/** A `{"t": "state"}` message from the bot: what the dashboard shows. */
export interface Snapshot {
  uptime: number;
  now: NowPlaying | null;
  queue: QueueItem[];
  queue_size: number;
  queue_requests?: number;
  radio_autoplay?: boolean;
  radio_lookahead?: Lookahead;
  counters: Counters;
  player: {
    paused: boolean;
    listeners: number;
    encoder_running: boolean;
    encoder_starts: number;
    next_ready?: boolean;
  };
  chat: { ready: boolean; subscribed: boolean; tokens?: Tokens };
  http: { up: boolean; base: string; stream: string; overlay: string; settings: string };
  workers: { alive: number; size: number };
  health?: { loop_lag_ms?: number };
}

export interface Preflight {
  ok?: boolean;
  config_ok?: boolean;
  config_error?: string;
  detail?: string;
  tokens?: Tokens;
  ffmpeg?: { path?: string | null };
  js_runtime?: { name?: string; path?: string | null };
  js_solver?: { ready?: boolean };
  ytdlp?: { version?: string; source?: string; bundled?: string; previous?: string | null; override_error?: string | null };
  tunables?: { bounds: Record<string, [number, number]>; defaults: Record<string, number> };
}

export interface Prefs {
  askOnClose: boolean;
  closeAction: 'tray' | 'quit';
  autoRestart: boolean;
  startBotOnLaunch: boolean;
  theme: 'system' | 'light' | 'dark';
  skin: string;
  launchAtLogin: boolean;
  checkAppUpdates: boolean;
  checkYtdlpOnStartup: boolean;
  lastUpdateCheck: number;
  lastYtdlpCheck: number;
  windowBg: string;
  windowDark: boolean;
}

export interface AppInfo {
  version: string;
  home: string;
  packaged: boolean;
  platform: 'windows' | 'linux' | 'macos';
  runtime: string;
}

export interface LogEntry {
  id: number;
  ts: number;
  level: string;
  logger: string;
  msg: string;
  cont: boolean;
}

export interface Ack {
  ok: boolean;
  error?: string;
  [key: string]: unknown;
}

export interface SearchResult {
  title: string;
  url: string;
  uploader?: string;
  duration?: number;
  thumbnail?: string;
}

export interface AppUpdate {
  ok: boolean;
  current: string;
  latest: string | null;
  updateAvailable: boolean;
  releaseUrl: string | null;
  installerUrl: string | null;
  error: string | null;
}

export interface YtdlpReport {
  ok: boolean;
  error?: string;
  current?: string;
  latest?: string | null;
  source?: string;
  bundled?: string;
  previous?: string | null;
  update_available?: boolean;
  override_error?: string | null;
}

export interface EnvData {
  values: Record<string, string>;
  secretSet: boolean;
  envPath: string;
}

export interface LiveData {
  tunables: Record<string, number>;
  bounds: Record<string, [number, number]>;
  radio_autoplay_enabled: boolean;
}

export interface SaveEnvResult {
  ok: boolean;
  error?: string;
  preflight?: Preflight;
  needsRestart?: boolean;
}
