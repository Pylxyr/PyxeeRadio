<script lang="ts">
  import { form } from '../../lib/form.svelte';

  let { id, key }: { id: string; key: string } = $props();
  let shown = $state(false);
  let input: HTMLInputElement | undefined = $state();

  function replace() {
    form.secretEditing = true;
    queueMicrotask(() => input?.focus());
  }
  function cancel() {
    form.secretEditing = false;
    form.values[key] = '';
  }
</script>

<div class="secret">
  {#if form.secretSet && !form.secretEditing}
    <span class="tag">set</span>
    <button class="btn ghost sm" type="button" onclick={replace}>Replace</button>
  {:else}
    <input
      bind:this={input}
      {id}
      class="input"
      type={shown ? 'text' : 'password'}
      autocomplete="off"
      spellcheck="false"
      value={form.values[key] ?? ''}
      oninput={(e) => (form.values[key] = e.currentTarget.value)}
    />
    <button class="btn ghost sm" type="button" onclick={() => (shown = !shown)}>{shown ? 'Hide' : 'Show'}</button>
    {#if form.secretSet}<button class="btn ghost sm" type="button" onclick={cancel}>Cancel</button>{/if}
  {/if}
</div>
