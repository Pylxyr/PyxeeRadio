// The Settings tab's form: what each setting is, and the checks run before a save. The shell checks
// the same things again (and refuses keys that are not listed), so this is for telling the person early.
import { OAUTH_REDIRECT } from './health';

export type FieldType = 'text' | 'secret' | 'number' | 'bool' | 'select';

export interface Field {
  key: string;
  label: string;
  type: FieldType;
  hint?: string;
  placeholder?: string;
  min?: number;
  max?: number;
  /** For ID fields: only digits. */
  digitsOnly?: boolean;
  options?: string[];
  def?: string | boolean;
}
export interface Special {
  special: 'cookies';
}
export interface Section {
  title: string;
  desc?: string;
  links?: { label: string; url?: string; copy?: string }[];
  fields: (Field | Special)[];
}

export const isField = (entry: Field | Special): entry is Field => 'key' in entry;

export const COOKIES_KEY = 'YTDLP_COOKIES_FILE';

export const SECTIONS: Section[] = [
  {
    title: 'Twitch',
    desc: 'Create a free app at the Twitch developer console, then paste its credentials. Account IDs are the numeric user IDs, not the names.',
    links: [
      { label: 'Open Twitch developer console', url: 'https://dev.twitch.tv/console/apps' },
      { label: 'Copy OAuth redirect URL', copy: OAUTH_REDIRECT },
    ],
    fields: [
      { key: 'TWITCH_CLIENT_ID', label: 'Client ID', type: 'text' },
      { key: 'TWITCH_CLIENT_SECRET', label: 'Client secret', type: 'secret' },
      { key: 'TWITCH_BOT_ID', label: 'Bot account ID', type: 'text', digitsOnly: true, hint: 'The account that posts in chat.' },
      { key: 'TWITCH_OWNER_ID', label: 'Broadcaster ID', type: 'text', digitsOnly: true, hint: 'Your channel.' },
    ],
  },
  {
    title: 'Audio & playback',
    fields: [
      { key: 'AUDIO_BITRATE_KBPS', label: 'Audio bitrate (kbps)', type: 'number', min: 64, max: 256, placeholder: '160', hint: 'Opus quality of the stream sent to OBS.' },
      {
        key: 'OVERLAY_DELAY_SECONDS', label: 'Overlay delay (seconds)', type: 'number', min: 0, max: 15, placeholder: '3',
        hint: 'The overlay and dashboard trail the player by this much, to match what you hear (OBS buffers the stream). It only makes up for OBS: a skip waits for the next song to be ready on its own. Raise it if they change before the sound does; lower it if they lag.',
      },
      { key: 'PAUSE_QUEUE_WHEN_NO_LISTENERS', label: 'Pause when nobody is listening', type: 'bool', def: false, hint: 'Holds the queue while OBS has no audio source connected.' },
    ],
  },
  {
    title: 'Network',
    desc: 'The audio stream, OBS overlay and web settings page are served on this computer only (127.0.0.1); nothing is reachable from your network.',
    fields: [{ key: 'TWITCH_NOWPLAYING_PORT', label: 'Port', type: 'number', min: 1024, max: 65535, placeholder: '8098' }],
  },
  {
    title: 'YouTube (yt-dlp)',
    fields: [
      { special: 'cookies' },
      { key: 'YTDLP_CONCURRENCY', label: 'Parallel lookups', type: 'number', min: 1, max: 8, placeholder: '2', hint: 'The most lookups that may run at once. Each runs in its own process (roughly 50-100 MB) that is started when needed and exits when idle.' },
      { key: 'YTDLP_WORKER_IDLE_SECONDS', label: 'Lookup process idle exit (s)', type: 'number', min: 15, max: 3600, placeholder: '120', hint: 'How long an unused lookup process stays loaded before it frees its memory.' },
      { key: 'YTDLP_EXTRACT_TIMEOUT_SECONDS', label: 'Lookup timeout (s)', type: 'number', min: 10, max: 120, placeholder: '45' },
      { key: 'YTDLP_CACHE_TTL_SECONDS', label: 'Result cache (s)', type: 'number', min: 0, max: 3600, placeholder: '900', hint: '0 turns caching off. Queued songs are still prepared ahead of time.' },
    ],
  },
  {
    title: 'Logging',
    fields: [
      { key: 'LOG_LEVEL', label: 'Log level', type: 'select', options: ['DEBUG', 'INFO', 'WARNING', 'ERROR'], def: 'INFO' },
      { key: 'LOG_TO_FILE', label: 'Also write a log file', type: 'bool', def: true },
    ],
  },
];

export const FIELDS: Field[] = SECTIONS.flatMap((s) => s.fields).filter(isField);

/** What the form holds for a value read from .env (booleans always spelled out, levels upper-case). */
export function storeValue(field: Field, raw: string): string {
  if (field.type === 'bool') return raw === '' ? String(field.def) : String(/^(1|true|yes|on)$/i.test(raw));
  if (field.type === 'select') return raw.toUpperCase() || String(field.def);
  return raw;
}

export type Values = Record<string, string>;

export const isDirty = (values: Values, orig: Values) => FIELDS.some((f) => (values[f.key] ?? '') !== (orig[f.key] ?? ''));

/** The problem with each field that has one, by key. */
export function validate(values: Values): Record<string, string> {
  const problems: Record<string, string> = {};
  for (const f of FIELDS) {
    const v = values[f.key] ?? '';
    if (f.type === 'number' && v !== '') {
      const n = Number(v);
      if (!Number.isInteger(n) || n < (f.min ?? -Infinity) || n > (f.max ?? Infinity)) {
        problems[f.key] = `Enter a whole number from ${f.min} to ${f.max}.`;
      }
    }
    if (f.digitsOnly && v && !/^\d+$/.test(v)) problems[f.key] = 'Digits only - this is the numeric Twitch user ID.';
  }
  return problems;
}

/** Only what changed, ready to save. */
export function changedValues(values: Values, orig: Values): Values {
  const changed: Values = {};
  for (const f of FIELDS) if ((values[f.key] ?? '') !== (orig[f.key] ?? '')) changed[f.key] = values[f.key] ?? '';
  return changed;
}
