<script lang="ts">
  import { toasts } from '../lib/store.svelte';
</script>

<div class="toasts" aria-live="polite">
  {#each toasts as t (t.id)}
    <div class="toast {t.kind}">{t.message}</div>
  {/each}
</div>
