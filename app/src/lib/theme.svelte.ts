// Light, dark or follow-the-system, plus the colour style on top. The shell keeps the native frame in
// step (window theme) and remembers the page's background so the next window starts in the right colour.
import { api } from './api';
import { backgroundOf, skinVariables, type Mode } from './skins';
import type { Prefs } from './types';

export const appearance = $state<{ mode: Mode; systemDark: boolean }>({ mode: 'dark', systemDark: true });

export const resolveMode = (theme: Prefs['theme'], systemDark: boolean): Mode =>
  theme === 'light' ? 'light' : theme === 'dark' ? 'dark' : systemDark ? 'dark' : 'light';

let skinVars: string[] = [];
let reported = '';

/** Puts the Appearance and Colour style settings on the page. */
export function applyAppearance(prefs: Pick<Prefs, 'theme' | 'skin'>, root: HTMLElement = document.documentElement): void {
  const mode = resolveMode(prefs.theme, appearance.systemDark);
  appearance.mode = mode;
  root.dataset.theme = mode;
  for (const name of skinVars) root.style.removeProperty(name);
  const vars = skinVariables(prefs.skin, mode);
  skinVars = Object.keys(vars);
  for (const [name, value] of Object.entries(vars)) root.style.setProperty(name, value);

  const bg = backgroundOf(prefs.skin, mode);
  const key = `${bg}/${mode}`;
  if (key !== reported) {
    reported = key;
    api.setAppearance(bg, mode === 'dark').catch(() => {});
  }
}

/** Follows the operating system's light/dark switch; `onChange` runs after it flips. Returns a stop function. */
export function watchSystemTheme(onChange: () => void): () => void {
  const query = window.matchMedia('(prefers-color-scheme: dark)');
  appearance.systemDark = query.matches;
  const handler = (event: MediaQueryListEvent) => {
    appearance.systemDark = event.matches;
    onChange();
  };
  query.addEventListener('change', handler);
  return () => query.removeEventListener('change', handler);
}
