<script lang="ts">
  import { logView } from '../lib/logs.svelte';
  import { bot, clock, goto, info, ui, type Tab } from '../lib/store.svelte';
  import { statusLabel } from '../lib/status';
  import Icon from './Icon.svelte';
  import type { IconName } from '../lib/icons';

  const tabs: { id: Tab; label: string; icon: IconName }[] = [
    { id: 'dashboard', label: 'Dashboard', icon: 'dashboard' },
    { id: 'logs', label: 'Logs', icon: 'logs' },
    { id: 'settings', label: 'Settings', icon: 'settings' },
  ];
  const badge = $derived(logView.unseenErrors > 99 ? '99+' : String(logView.unseenErrors));
</script>

<nav class="sidebar" aria-label="Main">
  {#each tabs as tab (tab.id)}
    <button class="nav" class:active={ui.tab === tab.id} aria-current={ui.tab === tab.id ? 'page' : undefined} onclick={() => goto(tab.id)}>
      <Icon name={tab.icon} /><span>{tab.label}</span>
      {#if tab.id === 'logs' && logView.unseenErrors > 0 && ui.tab !== 'logs'}<b class="badge">{badge}</b>{/if}
    </button>
  {/each}
  <div class="side-foot">
    <div class="muted small">{statusLabel(bot.status, clock.now)}</div>
    <div class="muted small">{info.version ? `v${info.version}` : ''}</div>
  </div>
</nav>
