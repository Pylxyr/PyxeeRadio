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

// The shell numbers every line, and the numbers only go up. It also hands each line over twice around the
// moment the window opens: once in the snapshot the page asks for, and again in the batch that was already
// on its way. Both copies must not get in: the rows are keyed by id, and a repeated key makes the Logs tab
// throw while it mounts, which leaves the Dashboard on screen under a highlighted "Logs". So a line is kept
// only if it is newer than everything seen before it. (Not reset by clearLogs(): a batch that was already
// in flight when Clear was pressed must not bring the cleared lines back.)
let newestId = 0;

function newer(entries: LogEntry[], after: number): { fresh: LogEntry[]; newest: number } {
  const fresh: LogEntry[] = [];
  let newest = after;
  for (const entry of entries) {
    if (entry.id > newest) {
      fresh.push(entry);
      newest = entry.id;
    }
  }
  return { fresh, newest };
}

export function setLogs(entries: LogEntry[]): void {
  const { fresh, newest } = newer(entries, 0);
  newestId = newest;
  logs.all = fresh.slice(-LOG_CAP);
}

export function addLogs(batch: LogEntry[], logsTabOpen: boolean): void {
  const { fresh, newest } = newer(batch, newestId);
  newestId = newest;
  if (!fresh.length) return;
  let unseen = 0;
  for (const entry of fresh) if (!entry.cont && levelKey(entry.level) === 'ERROR' && !logsTabOpen) unseen++;
  const next = logs.all.concat(fresh);
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
