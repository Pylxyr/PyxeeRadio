import { describe, expect, it } from 'vitest';
import { FIELDS, SECTIONS, changedValues, isDirty, storeValue, validate } from '../lib/settings';

const field = (key: string) => FIELDS.find((f) => f.key === key)!;

describe('settings form', () => {
  it('every setting the page can write is a key the shell accepts', () => {
    // The list in src-tauri/src/config.rs (ENV_KEYS) is what the shell lets through; keep the two the same.
    const rust = readRustKeys();
    const mine = [...FIELDS.map((f) => f.key), 'YTDLP_COOKIES_FILE'].sort();
    expect(mine).toEqual([...new Set(mine)]);
    expect(mine).toEqual(rust.sort());
  });

  it('numbers must be whole and in range', () => {
    expect(validate({ AUDIO_BITRATE_KBPS: '160' })).toEqual({});
    expect(validate({ AUDIO_BITRATE_KBPS: '' })).toEqual({}); // empty means "use the default"
    for (const bad of ['10', '999', '1.5', 'abc']) expect(validate({ AUDIO_BITRATE_KBPS: bad }).AUDIO_BITRATE_KBPS).toContain('whole number from 64 to 256');
  });

  it('account IDs are digits only', () => {
    expect(validate({ TWITCH_BOT_ID: '12345' })).toEqual({});
    expect(validate({ TWITCH_BOT_ID: 'bot_account' }).TWITCH_BOT_ID).toContain('Digits only');
  });

  it('booleans are always spelled out and log levels upper-cased when loaded', () => {
    const flag = field('LOG_TO_FILE');
    expect(storeValue(flag, '')).toBe('true');
    expect(storeValue(flag, 'off')).toBe('false');
    expect(storeValue(flag, 'Yes')).toBe('true');
    expect(storeValue(field('LOG_LEVEL'), 'debug')).toBe('DEBUG');
    expect(storeValue(field('LOG_LEVEL'), '')).toBe('INFO');
  });

  it('dirty tracking sends only what changed', () => {
    const orig = { TWITCH_CLIENT_ID: 'a', LOG_LEVEL: 'INFO' };
    expect(isDirty(orig, orig)).toBe(false);
    const edited = { ...orig, LOG_LEVEL: 'DEBUG' };
    expect(isDirty(edited, orig)).toBe(true);
    expect(changedValues(edited, orig)).toEqual({ LOG_LEVEL: 'DEBUG' });
    expect(changedValues({ ...orig, TWITCH_CLIENT_SECRET: 's' }, orig)).toEqual({ TWITCH_CLIENT_SECRET: 's' });
  });

  it('every section has a title and at least one field', () => {
    for (const section of SECTIONS) {
      expect(section.title).not.toBe('');
      expect(section.fields.length).toBeGreaterThan(0);
    }
  });
});

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

function readRustKeys(): string[] {
  const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'src-tauri', 'src', 'config.rs'), 'utf8');
  const block = /pub const ENV_KEYS[^=]*= \[([\s\S]*?)\];/.exec(source)?.[1] ?? '';
  return [...block.matchAll(/"([A-Z0-9_]+)"/g)].map((m) => m[1]!);
}
