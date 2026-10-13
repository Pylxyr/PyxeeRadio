<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';

  const win = getCurrentWindow();
  let maximized = $state(false);

  async function sync() {
    maximized = await win.isMaximized().catch(() => false);
  }
  $effect(() => {
    void sync();
    window.addEventListener('resize', sync);
    return () => window.removeEventListener('resize', sync);
  });
</script>

<div class="winctl">
  <button type="button" aria-label="Minimize" title="Minimize" onclick={() => win.minimize()}>
    <svg viewBox="0 0 10 10"><path d="M0 5h10" /></svg>
  </button>
  <button type="button" aria-label={maximized ? 'Restore' : 'Maximize'} title={maximized ? 'Restore' : 'Maximize'} onclick={() => win.toggleMaximize()}>
    {#if maximized}
      <svg viewBox="0 0 10 10"><path d="M2.5 2.5V0.5h7v7h-2M0.5 2.5h7v7h-7z" /></svg>
    {:else}
      <svg viewBox="0 0 10 10"><path d="M0.5 0.5h9v9h-9z" /></svg>
    {/if}
  </button>
  <button type="button" class="close" aria-label="Close" title="Close" onclick={() => win.close()}>
    <svg viewBox="0 0 10 10"><path d="M0 0l10 10M10 0L0 10" /></svg>
  </button>
</div>
