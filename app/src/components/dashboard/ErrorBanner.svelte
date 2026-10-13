<script lang="ts">
  import { bot, goto } from '../../lib/store.svelte';

  const fatal = $derived(bot.status.phase === 'error' || bot.status.fatal ? bot.status.fatal : null);
  const phase = $derived(bot.status.phase);
</script>

{#if phase === 'error' && fatal}
  <div class="banner err" role="alert">
    <div><strong>{fatal.message}</strong><div class="small">{fatal.hint}</div></div>
    <div class="row">
      <button class="btn ghost" onclick={() => goto('logs')}>View logs</button>
      <button class="btn ghost" onclick={() => goto('settings')}>Open Settings</button>
    </div>
  </div>
{/if}
