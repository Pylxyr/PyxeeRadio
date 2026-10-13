<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from './lib/api';
  import { init, ui } from './lib/store.svelte';
  import CloseDialog from './components/CloseDialog.svelte';
  import Dashboard from './components/dashboard/Dashboard.svelte';
  import LogsTab from './components/LogsTab.svelte';
  import SettingsTab from './components/SettingsTab.svelte';
  import Sidebar from './components/Sidebar.svelte';
  import TitleBar from './components/TitleBar.svelte';
  import Toasts from './components/Toasts.svelte';

  onMount(() => {
    let stop: (() => void) | undefined;
    let gone = false;
    init()
      .then((cleanup) => {
        if (gone) return cleanup();
        stop = cleanup;
        // The window is created hidden (so it never flashes the wrong colour); show it once this has painted.
        requestAnimationFrame(() => requestAnimationFrame(() => void api.uiReady()));
      })
      .catch(() => void api.uiReady());
    return () => {
      gone = true;
      stop?.();
    };
  });
</script>

<TitleBar />
<div class="shell">
  <Sidebar />
  <main>
    {#if ui.ready}
      {#if ui.tab === 'dashboard'}<Dashboard />{:else if ui.tab === 'logs'}<LogsTab />{:else}<SettingsTab />{/if}
    {/if}
  </main>
</div>
<Toasts />
{#if ui.closeAsk}<CloseDialog />{/if}
