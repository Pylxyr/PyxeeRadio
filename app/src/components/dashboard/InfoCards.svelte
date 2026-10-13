<script lang="ts">
  import { api } from '../../lib/api';
  import { healthItems } from '../../lib/health';
  import { bot, copyText, isLive } from '../../lib/store.svelte';

  const live = $derived(isLive());
  const items = $derived(healthItems({ live, snap: bot.snap, pre: bot.pre }));
  const c = $derived(live && bot.snap ? bot.snap.counters : null);
  const tiles = $derived<[string, number | null][]>([
    ['Requests queued', c?.requests_queued ?? null],
    ['Songs played', c?.tracks_played ?? null],
    ['Skipped', c?.skips ?? null],
    ['Failed to load', c?.tracks_failed ?? null],
    ['Links resolved', c?.resolve_success ?? null],
    ['Resolve failures', c?.resolve_failure ?? null],
  ]);
  const http = $derived(bot.snap?.http);
  const stream = $derived(http?.stream ?? 'http://localhost:8098/stream.opus');
  const overlay = $derived(http?.overlay ?? 'http://localhost:8098/overlay');
  const settings = $derived(http?.settings ?? 'http://localhost:8098/settings');
</script>

<div class="grid2" style="grid-template-columns: 1fr">
  <div class="card">
    <div class="card-head"><h2>Connections &amp; health</h2></div>
    <ul class="health">
      {#each items as item (item.name)}
        <li class={item.state}><span class="hdot"></span><span class="hname">{item.name}</span><span class="hval">{item.value}</span></li>
      {/each}
    </ul>
  </div>
</div>

<div class="grid2">
  <div class="card">
    <div class="card-head"><h2>Activity <span class="muted">· last hour</span></h2></div>
    <div class="counters">
      {#each tiles as [label, value] (label)}
        <div class="counter"><b>{value ?? '–'}</b><span>{label}</span></div>
      {/each}
    </div>
  </div>
  <div class="card">
    <div class="card-head"><h2>Add to OBS</h2></div>
    <div class="obs">
      <div class="obs-row">
        <div><div class="obs-name">Audio source <span class="muted small">Media Source · uncheck “Local file”</span></div><code>{stream}</code></div>
        <button class="btn ghost sm" onclick={() => copyText(stream, 'Stream URL copied')}>Copy</button>
      </div>
      <div class="obs-row">
        <div><div class="obs-name">Now-playing overlay <span class="muted small">Browser Source</span></div><code>{overlay}</code></div>
        <button class="btn ghost sm" onclick={() => copyText(overlay, 'Overlay URL copied')}>Copy</button>
      </div>
      <div class="obs-row">
        <div><div class="obs-name">Web settings page <span class="muted small">optional, in your browser</span></div><code>{settings}</code></div>
        <button class="btn ghost sm" onclick={() => api.openExternal(settings)}>Open</button>
      </div>
    </div>
  </div>
</div>
