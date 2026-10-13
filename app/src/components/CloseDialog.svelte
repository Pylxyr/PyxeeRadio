<script lang="ts">
  import { api } from '../lib/api';
  import { ui } from '../lib/store.svelte';

  let remember = $state(false);
  let first: HTMLButtonElement | undefined = $state();

  $effect(() => first?.focus());

  function answer(action: 'tray' | 'quit' | 'cancel') {
    ui.closeAsk = false;
    void api.closeDecision(action, action !== 'cancel' && remember);
    remember = false;
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && answer('cancel')} />

<div class="modal-backdrop" role="presentation">
  <div class="modal" role="alertdialog" aria-modal="true" aria-labelledby="close-title" aria-describedby="close-desc">
    <h3 id="close-title">PyxeeRadio is still running</h3>
    <p id="close-desc">
      The radio is live. Keeping it in the tray leaves the music and chat bot running. Stopping shuts everything down cleanly.
    </p>
    <label class="check"><input type="checkbox" bind:checked={remember} /> Don't ask again</label>
    <div class="actions">
      <button class="btn ghost" onclick={() => answer('cancel')}>Cancel</button>
      <button class="btn" onclick={() => answer('quit')}>Stop and quit</button>
      <button class="btn primary" bind:this={first} onclick={() => answer('tray')}>Keep running in tray</button>
    </div>
  </div>
</div>
