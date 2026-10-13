<script lang="ts">
  import { api } from '../../lib/api';
  import { copyText } from '../../lib/store.svelte';
  import type { Section } from '../../lib/settings';
  import FieldRow from './FieldRow.svelte';

  let { section }: { section: Section } = $props();
</script>

<div class="card">
  <div class="card-head"><h2>{section.title}</h2></div>
  {#if section.desc}<div class="sec-desc">{section.desc}</div>{/if}
  {#if section.links}
    <div class="links" style="margin-bottom: 10px">
      {#each section.links as link (link.label)}
        <button class="btn ghost sm" type="button" onclick={() => (link.url ? api.openExternal(link.url) : copyText(link.copy ?? '', 'Copied'))}>{link.label}</button>
      {/each}
    </div>
  {/if}
  {#each section.fields as entry, i (i)}<FieldRow {entry} />{/each}
</div>
