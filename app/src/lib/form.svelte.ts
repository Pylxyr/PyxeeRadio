// The Settings tab's working state. It lives here, not in the component, so a half-edited form
// survives a switch to another tab and back.
import { api } from './api';
import { COOKIES_KEY, FIELDS, changedValues, isDirty, storeValue, validate, type Values } from './settings';
import { bot, isLive, refreshPreflight, toast } from './store.svelte';
import type { LiveData } from './types';

export const form = $state({
  values: {} as Values,
  orig: {} as Values,
  secretSet: false,
  secretEditing: false,
  envPath: '',
  loaded: false,
  problems: {} as Record<string, string>,
  saveError: '',
  saving: false,
  restartNeeded: false,
});

export const live = $state<{ data: LiveData | null; tunables: Record<string, string>; radio: boolean }>({
  data: null,
  tunables: {},
  radio: true,
});

export const dirty = () => isDirty(form.values, form.orig);

export async function loadSettings(): Promise<void> {
  try {
    const [env, liveData] = await Promise.all([api.getEnv(), api.getLive()]);
    if (!dirty()) {
      form.secretSet = !!env.secretSet;
      form.secretEditing = false;
      form.envPath = env.envPath;
      const values: Values = {};
      for (const f of FIELDS) values[f.key] = storeValue(f, env.values[f.key] ?? '');
      values[COOKIES_KEY] = env.values[COOKIES_KEY] ?? '';
      form.values = values;
      form.orig = { ...values };
    }
    live.data = liveData;
    live.tunables = Object.fromEntries(Object.entries(liveData.tunables).map(([k, v]) => [k, String(v)]));
    live.radio = liveData.radio_autoplay_enabled;
    form.loaded = true;
  } catch (error) {
    toast(`Couldn't load settings: ${error instanceof Error ? error.message : String(error)}`, 'err');
  }
}

export function discard(): void {
  form.values = { ...form.orig };
  form.secretEditing = false;
  form.problems = {};
  form.saveError = '';
}

export async function saveSettings(): Promise<void> {
  form.problems = validate(form.values);
  if (Object.keys(form.problems).length) {
    form.saveError = 'Fix the highlighted fields first.';
    return;
  }
  form.saveError = '';
  form.saving = true;
  try {
    const result = await api.saveEnv({ values: changedValues(form.values, form.orig) });
    if (!result.ok) {
      form.saveError = result.error || "Couldn't save.";
      return;
    }
    if (result.preflight) bot.pre = result.preflight;
    form.values.TWITCH_CLIENT_SECRET = '';
    form.orig = { ...form.values };
    form.restartNeeded = !!result.needsRestart;
    toast('Settings saved.', 'ok');
    await loadSettings();
  } catch (error) {
    form.saveError = error instanceof Error ? error.message : String(error);
  } finally {
    form.saving = false;
  }
}

export async function applyLive(): Promise<void> {
  if (!live.data) return;
  try {
    const result = await api.saveLive({ tunables: { ...live.tunables }, radio_autoplay_enabled: live.radio });
    live.data = result;
    live.tunables = Object.fromEntries(Object.entries(result.tunables).map(([k, v]) => [k, String(v)]));
    live.radio = result.radio_autoplay_enabled;
    toast(isLive() ? 'Applied. The running bot picks this up right away.' : 'Saved. Applies on next start.', 'ok');
  } catch (error) {
    toast(`Couldn't save: ${error instanceof Error ? error.message : String(error)}`, 'err');
  }
}

export async function restartNow(): Promise<void> {
  form.restartNeeded = false;
  await api.restart();
  await refreshPreflight(true);
}
