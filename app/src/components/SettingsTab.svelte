<script lang="ts">
  import { dirty, discard, form, loadSettings, restartNow, saveSettings } from '../lib/form.svelte';
  import { SECTIONS } from '../lib/settings';
  import AppCard from './settings/AppCard.svelte';
  import LiveCard from './settings/LiveCard.svelte';
  import SectionCard from './settings/SectionCard.svelte';
  import UpdatesCard from './settings/UpdatesCard.svelte';

  $effect(() => {
    void loadSettings();
  });
  const changed = $derived(form.loaded && dirty());
</script>

<section class="tab" aria-label="Settings">
  {#if form.restartNeeded}
    <div class="banner warn">
      <div><strong>Saved.</strong> Restart the bot for these changes to take effect.</div>
      <button class="btn primary sm" onclick={restartNow}>Restart now</button>
    </div>
  {/if}
  {#if form.loaded}
    <div class="settings">
      {#each SECTIONS as section (section.title)}<SectionCard {section} />{/each}
      <LiveCard />
      <AppCard />
      <UpdatesCard />
    </div>
  {/if}
  {#if changed}
    <div class="savebar">
      <span>You have unsaved changes.</span>
      <div class="row">
        <span class="err-text small" role="alert">{form.saveError}</span>
        <button class="btn ghost" onclick={discard}>Discard</button>
        <button class="btn primary" disabled={form.saving} onclick={saveSettings}>{form.saving ? 'Saving…' : 'Save changes'}</button>
      </div>
    </div>
  {/if}
</section>
