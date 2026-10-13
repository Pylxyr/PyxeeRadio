import { describe, expect, it } from 'vitest';
import { fmtDur, fmtUptime, looksLikeLink, splitMatches } from '../lib/format';

describe('format', () => {
  it('fmtDur: minutes and seconds, hours from an hour up, never negative', () => {
    expect(fmtDur(0)).toBe('0:00');
    expect(fmtDur(65)).toBe('1:05');
    expect(fmtDur(3599)).toBe('59:59');
    expect(fmtDur(3723)).toBe('1:02:03');
    expect(fmtDur(-5)).toBe('0:00');
    expect(fmtDur(null)).toBe('0:00');
    expect(fmtDur(61.9)).toBe('1:01');
  });

  it('fmtUptime picks the two most useful units', () => {
    expect(fmtUptime(5)).toBe('0m 05s');
    expect(fmtUptime(125)).toBe('2m 05s');
    expect(fmtUptime(2 * 3600 + 7 * 60)).toBe('2h 07m');
    expect(fmtUptime(3 * 86400 + 4 * 3600)).toBe('3d 4h');
  });

  it('a link is anything starting with http(s)://', () => {
    expect(looksLikeLink('https://youtu.be/x')).toBe(true);
    expect(looksLikeLink('HTTP://x')).toBe(true);
    expect(looksLikeLink('daft punk')).toBe(false);
    expect(looksLikeLink('ftp://x')).toBe(false);
  });

  it('splitMatches cuts text around every match, ignoring case, and keeps all of it', () => {
    expect(splitMatches('Hello hello', '')).toEqual([{ text: 'Hello hello', match: false }]);
    const parts = splitMatches('Error: error!', 'error');
    expect(parts).toEqual([
      { text: 'Error', match: true },
      { text: ': ', match: false },
      { text: 'error', match: true },
      { text: '!', match: false },
    ]);
    expect(splitMatches('nothing', 'zzz')).toEqual([{ text: 'nothing', match: false }]);
    expect(parts.map((p) => p.text).join('')).toBe('Error: error!');
  });
});
