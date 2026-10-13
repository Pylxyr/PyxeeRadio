<script lang="ts">
  import { api } from '../../lib/api';
  import { SKINS, SKIN_NAMES, preview } from '../../lib/skins';
  import { info, prefs, setPrefs } from '../../lib/store.svelte';
  import { appearance } from '../../lib/theme.svelte';
  import type { Prefs } from '../../lib/types';

  type BoolKey = 'askOnClose' | 'autoRestart' | 'startBotOnLaunch' | 'launchAtLogin';
  const toggles = $derived<[BoolKey, string, string][]>(
    [
      ['askOnClose', 'Ask before closing while the radio is running', 'Otherwise the window follows your last choice.'],
      ['autoRestart', 'Restart automatically if the bot crashes', 'Up to 5 times in 10 minutes. Not for settings errors.'],
      ['startBotOnLaunch', 'Start the bot when this app opens', ''],
      [
        'launchAtLogin',
        `Open PyxeeRadio when I sign in to ${info.platform === 'windows' ? 'Windows' : 'my desktop'}`,
        'Starts minimized to the tray.',
      ],
    ].filter(([key]) => key !== 'launchAtLogin' || info.packaged) as [BoolKey, string, string][],
  );
  const themes: [Prefs['theme'], string][] = [['system', 'Match my system'], ['light', 'Light'], ['dark', 'Dark']];
</script>

<div class="card">
  <div class="card-head"><h2>This app</h2></div>
  <div class="field">
    <label class="name" for="p-theme">Appearance</label>
    <div class="ctl">
      <select id="p-theme" class="input" value={prefs.theme} onchange={(e) => setPrefs({ theme: e.currentTarget.value as Prefs['theme'] })}>
        {#each themes as [value, label] (value)}<option {value}>{label}</option>{/each}
      </select>
      <div class="hint">Light or dark, or follow {info.platform === 'windows' ? 'Windows' : 'your desktop'}.</div>
    </div>
  </div>
  <div class="field">
    <span class="name">Color style</span>
    <div class="ctl">
      <div class="skin-grid" role="group" aria-label="Color style">
        {#each SKIN_NAMES as name (name)}
          {@const look = preview(name, appearance.mode)}
          <button
            class="skin"
            type="button"
            aria-pressed={prefs.skin === name}
            title={SKINS[name]?.label}
            style:--sw-bg={look.bg}
            style:--sw-panel={look.panel}
            style:--sw-accent={look.accent}
            onclick={() => setPrefs({ skin: name })}
          >
            <span class="sw"><i></i></span><span>{SKINS[name]?.label}</span>
          </button>
        {/each}
      </div>
      <div class="hint">Tints the whole window, in light or dark.</div>
    </div>
  </div>
  {#each toggles as [key, label, hint] (key)}
    <div class="field">
      <label class="name" for="p-{key}">{label}</label>
      <div class="ctl">
        <label class="switch"><input id="p-{key}" type="checkbox" checked={prefs[key]} onchange={(e) => setPrefs({ [key]: e.currentTarget.checked })} /><span></span></label>
        {#if hint}<div class="hint">{hint}</div>{/if}
      </div>
    </div>
  {/each}
  <div class="field">
    <span class="name">Files</span>
    <div class="ctl">
      <div class="links">
        <button class="btn ghost sm" type="button" onclick={() => api.openPath('data')}>Open data folder</button>
        <button class="btn ghost sm" type="button" onclick={() => api.openPath('logs')}>Open log folder</button>
        <button class="btn ghost sm" type="button" onclick={() => api.openPath('env')}>Show .env file</button>
      </div>
      <div class="hint">Settings and data live in {info.home || ''}. Uninstalling keeps them.</div>
    </div>
  </div>
</div>
