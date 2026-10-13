// What the dashboard says about the bot's connections and about the setup: plain functions of the
// last state report and the last preflight, so they are easy to test.
import type { Preflight, Snapshot, Tokens } from './types';

export const OAUTH_BOT = 'http://localhost:4343/oauth?scopes=user:read:chat+user:write:chat+user:bot&force_verify=true';
export const OAUTH_OWNER = 'http://localhost:4343/oauth?scopes=channel:bot&force_verify=true';
export const OAUTH_REDIRECT = 'http://localhost:4343/oauth/callback';

export type HealthState = 'ok' | 'warn' | 'err' | 'idle';
export interface HealthItem {
  name: string;
  state: HealthState;
  value: string;
}

export interface Inputs {
  live: boolean;
  snap: Snapshot | null;
  pre: Preflight | null;
}

export function tokensOf({ snap, pre }: Pick<Inputs, 'snap' | 'pre'>): Tokens {
  const fromSnap = snap?.chat?.tokens && Object.keys(snap.chat.tokens).length ? snap.chat.tokens : null;
  return fromSnap ?? pre?.tokens ?? {};
}

export const solverReady = (pre: Preflight | null) => !!pre?.js_solver?.ready;

export function healthItems({ live, snap: s, pre }: Inputs): HealthItem[] {
  const tok = tokensOf({ snap: s, pre });
  const items: HealthItem[] = [];
  const add = (name: string, state: HealthState, value: string) => items.push({ name, state, value });

  if (!live) add('Twitch chat', 'idle', 'Offline');
  else if (s?.chat.ready && s.chat.subscribed) add('Twitch chat', 'ok', 'Connected');
  else if (s?.chat.ready) add('Twitch chat', 'warn', 'Waiting for sign-in');
  else add('Twitch chat', 'warn', 'Connecting…');

  const signIn = (name: string, ok: boolean) => {
    if (tok.readable === false) add(name, 'err', 'Token file unreadable');
    else add(name, ok ? 'ok' : pre?.config_ok ? 'warn' : 'idle', ok ? 'Authorized' : 'Not authorized');
  };
  signIn('Bot account sign-in', !!tok.bot);
  signIn('Broadcaster sign-in', !!tok.owner);

  if (!live) add('Audio encoder', 'idle', 'Idle');
  else if (s?.player.encoder_running) {
    const restarted = s.player.encoder_starts > 1;
    add('Audio encoder', restarted ? 'warn' : 'ok', restarted ? `Restarted ${s.player.encoder_starts - 1}×` : 'Running');
  } else add('Audio encoder', 'idle', 'Starts with the first song');

  if (!live) add('Web server', 'idle', 'Offline');
  else if (s?.http.up) add('Web server', 'ok', `${new URL(s.http.base).host} (this PC only)`);
  else add('Web server', 'warn', 'Starting…');

  const w = s?.workers;
  if (!live || !w) add('yt-dlp workers', 'idle', 'Offline');
  else add('yt-dlp workers', 'ok', w.alive ? `${w.alive} running (up to ${w.size})` : 'Start on demand');

  if (!pre) add('ffmpeg', 'idle', 'Checking…');
  else add('ffmpeg', pre.ffmpeg?.path ? 'ok' : 'err', pre.ffmpeg?.path ? 'Found' : 'Not found');
  if (!pre) add('JS runtime', 'idle', 'Checking…');
  else {
    const js = pre.js_runtime ?? {};
    add(js.name ? `JS runtime (${js.name})` : 'JS runtime', js.path ? 'ok' : 'warn', js.path ? 'Found' : 'Not found');
  }
  if (pre) add('YouTube JS solver', solverReady(pre) ? 'ok' : 'idle', solverReady(pre) ? 'Ready' : 'Not needed so far');
  if (live && s) {
    const lag = s.health?.loop_lag_ms ?? 0;
    add('Event-loop delay', lag < 60 ? 'ok' : lag < 250 ? 'warn' : 'err', `${Math.round(lag)} ms`);
  }
  return items;
}

export interface SetupStep {
  id: 'credentials' | 'bot' | 'owner';
  done: boolean;
  title: string;
  desc: string;
  /** For the authorize steps: the link to open and whether the bot has to be running for it. */
  url?: string;
  needsBot?: boolean;
}

export function setupSteps({ live, snap, pre }: Inputs): SetupStep[] {
  const tok = tokensOf({ snap, pre });
  const cfg = !!pre?.config_ok;
  return [
    {
      id: 'credentials',
      done: cfg,
      title: 'Enter your Twitch app credentials',
      desc: cfg ? '' : `Client ID, secret and the two account IDs. In your Twitch app, set the OAuth Redirect URL to ${OAUTH_REDIRECT}`,
    },
    {
      id: 'bot',
      done: !!tok.bot,
      title: 'Authorize the bot account',
      desc: live ? 'Opens Twitch in your browser. Sign in as the BOT account.' : 'Press Start first - the sign-in page is served by the running bot.',
      url: OAUTH_BOT,
      needsBot: true,
    },
    {
      id: 'owner',
      done: !!tok.owner,
      title: 'Authorize your broadcaster account',
      desc: 'Do this in a private window, or after signing out of the bot account on twitch.tv.',
      url: OAUTH_OWNER,
      needsBot: true,
    },
  ];
}

/** "2 requested · 5 from the radio mix", or why the queue is not moving. */
export function queueSummary(s: Snapshot): string {
  if (s.player.paused) return 'Playback paused';
  const radio = Math.max(0, s.queue_size - (s.queue_requests ?? s.queue_size));
  const requested = s.queue_size - radio;
  if (s.radio_autoplay === false) return `${requested} requested · auto-radio off`;
  return `${requested} requested · ${radio} from radio`;
}
