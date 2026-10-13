const pad = (n: number, width = 2) => String(n).padStart(width, '0');

/** 3:07, or 1:02:03 from an hour up. */
export function fmtDur(seconds: number | null | undefined): string {
  const total = Math.max(0, Math.floor(seconds || 0));
  const hh = Math.floor(total / 3600);
  const mm = Math.floor((total % 3600) / 60);
  const ss = total % 60;
  return hh ? `${hh}:${pad(mm)}:${pad(ss)}` : `${mm}:${pad(ss)}`;
}

/** 5m 03s, 2h 07m, 3d 4h. */
export function fmtUptime(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const d = Math.floor(total / 86400);
  const hh = Math.floor((total % 86400) / 3600);
  const mm = Math.floor((total % 3600) / 60);
  if (d) return `${d}d ${hh}h`;
  if (hh) return `${hh}h ${pad(mm)}m`;
  return `${mm}m ${pad(total % 60)}s`;
}

/** 12:34:56.789, local time, for a log line. */
export function stamp(ts: number): string {
  const d = new Date(ts * 1000);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${pad(d.getMilliseconds(), 3)}`;
}

export const looksLikeLink = (text: string) => /^https?:\/\//i.test(text);

/** `text` cut into plain and matching parts, for highlighting a search in the logs. */
export function splitMatches(text: string, query: string): { text: string; match: boolean }[] {
  if (!query) return [{ text, match: false }];
  const out: { text: string; match: boolean }[] = [];
  const lower = text.toLowerCase();
  const q = query.toLowerCase();
  let from = 0;
  for (let at = lower.indexOf(q); at !== -1; at = lower.indexOf(q, from)) {
    if (at > from) out.push({ text: text.slice(from, at), match: false });
    out.push({ text: text.slice(at, at + q.length), match: true });
    from = at + q.length;
  }
  if (from < text.length) out.push({ text: text.slice(from), match: false });
  return out.length ? out : [{ text, match: false }];
}
