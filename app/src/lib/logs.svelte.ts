// The Logs tab's data: the newest lines, the filter on them, and how many errors came in unseen.
import type { LogEntry } from './types';

export const LOG_CAP = 5000;

export const levelKey = (level: string) => (level === 'CRITICAL' ? 'ERROR' : level);
export const LEVELS = ['DEBUG', 'INFO', 'WARNING', 'ERROR'] as const;

// The lines are not made deeply reactive (5,000 proxied objects would cost real memory); replacing the
// whole array is what notifies. That has to be a $state.raw *field*: a $state.raw object whose property is
// reassigned notifies nobody.
class LogStore {
  all = $state.raw<LogEntry[]>([]);
}
export const logs = new LogStore();
export const logView = $state({ levels: new Set<string>(LEVELS), query: '', follow: true, unseenErrors: 0 });

export function setLogs(entries: LogEntry[]): void {
  logs.all = entries.slice(-LOG_CAP);
}

export function addLogs(batch: LogEntry[], logsTabOpen: boolean): void {
  let unseen = 0;
  for (const entry of batch) if (!entry.cont && levelKey(entry.level) === 'ERROR' && !logsTabOpen) unseen++;
  const next = logs.all.concat(batch);
  logs.all = next.length > LOG_CAP ? next.slice(next.length - LOG_CAP) : next;
  logView.unseenErrors += unseen;
}

export function resetUnseen(): void {
  logView.unseenErrors = 0;
}

export function clearLogs(): void {
  logs.all = [];
  logView.unseenErrors = 0;
}

/** The lines that pass the level chips and the search box. */
export function filterLogs(all: LogEntry[], levels: Set<string>, query: string): LogEntry[] {
  const q = query.trim().toLowerCase();
  return all.filter((e) => levels.has(levelKey(e.level)) && (!q || e.msg.toLowerCase().includes(q) || e.logger.toLowerCase().includes(q)));
}
