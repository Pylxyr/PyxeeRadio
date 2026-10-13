import type { BotStatus, Phase } from './types';

export const PHASE_LABEL: Record<Phase, string> = {
  stopped: 'Stopped',
  starting: 'Starting…',
  running: 'Running',
  stopping: 'Stopping…',
  error: 'Error',
};

/** What the status pill says: the phase, or the countdown to an automatic restart. */
export function statusLabel(status: Pick<BotStatus, 'phase' | 'restartAt'>, now: number): string {
  if (status.restartAt) return `Restarting in ${Math.max(0, Math.ceil((status.restartAt - now) / 1000))}s`;
  return PHASE_LABEL[status.phase] ?? status.phase;
}
