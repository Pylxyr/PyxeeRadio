<script lang="ts">
  import { api } from '../lib/api';
  import logo from '../assets/logo.png';
  import { bot, clock, info, prefs, setPrefs, startBot } from '../lib/store.svelte';
  import { statusLabel } from '../lib/status';
  import { appearance } from '../lib/theme.svelte';
  import Icon from './Icon.svelte';
  import WindowControls from './WindowControls.svelte';

  const phase = $derived(bot.status.phase);
  const live = $derived(phase === 'starting' || phase === 'running');
  const label = $derived(statusLabel(bot.status, clock.now));
  const dark = $derived(appearance.mode === 'dark');
  const themeLabel = $derived(dark ? 'Switch to light mode' : 'Switch to dark mode');

  const toggleTheme = () => setPrefs({ theme: dark ? 'light' : 'dark' });
  void prefs;
</script>

<header class="titlebar" data-tauri-drag-region>
  <div class="brand" data-tauri-drag-region>
    <img src={logo} alt="" data-tauri-drag-region />
    <span data-tauri-drag-region>PyxeeRadio</span>
  </div>
  <div class="tb-controls">
    <button type="button" class="icon-btn" aria-label={themeLabel} title={themeLabel} onclick={toggleTheme}>
      <Icon name={dark ? 'sun' : 'moon'} />
    </button>
    <button
      type="button"
      class="icon-btn"
      aria-label="Hide to tray"
      title="Hide to tray: the window closes and the app keeps running in the background"
      onclick={() => api.hideToTray()}
    >
      <Icon name="tray" />
    </button>
    <div class="pill {phase}" role="status"><span class="dot"></span><span>{label}</span></div>
    {#if !(live || phase === 'stopping')}
      <button class="btn primary" onclick={startBot}>{phase === 'error' ? 'Try again' : 'Start'}</button>
    {:else}
      <button class="btn danger" disabled={phase === 'stopping'} onclick={() => api.stop()}>{phase === 'stopping' ? 'Stopping…' : 'Stop'}</button>
    {/if}
    {#if live}
      <button class="btn ghost" onclick={() => api.restart()}>Restart</button>
    {/if}
    {#if info.platform === 'windows'}<WindowControls />{/if}
  </div>
</header>
