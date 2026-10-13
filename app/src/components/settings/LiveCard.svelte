<script lang="ts">
  import { applyLive, live } from '../../lib/form.svelte';

  const defs: [string, string, string][] = [
    ['max_pending_per_chatter', 'Max pending per viewer', 'How many songs one viewer can have queued.'],
    ['request_cooldown_seconds', 'Request cooldown (s)', '0 disables the cooldown.'],
    ['queue_cap', 'Queue cap', 'Total requests before !sr turns people away.'],
    ['max_request_duration_seconds', 'Max track length (s)', 'Longer songs are refused.'],
  ];
</script>

{#if live.data}
  <div class="card">
    <div class="card-head"><h2>Live limits</h2><span class="muted small">Apply instantly - no restart</span></div>
    {#each defs as [key, label, hint] (key)}
      {@const range = live.data.bounds[key]}
      {#if range}
        <div class="field">
          <label class="name" for="l-{key}">{label}</label>
          <div class="ctl">
            <input id="l-{key}" class="input num" type="number" min={range[0]} max={range[1]} value={live.tunables[key] ?? ''} oninput={(e) => (live.tunables[key] = e.currentTarget.value)} />
            <div class="hint">{hint} ({range[0]}–{range[1]})</div>
          </div>
        </div>
      {/if}
    {/each}
    <div class="field">
      <label class="name" for="l-radio">Auto-radio</label>
      <div class="ctl">
        <label class="switch"><input id="l-radio" type="checkbox" checked={live.radio} onchange={(e) => (live.radio = e.currentTarget.checked)} /><span></span></label>
        <div class="hint">Queue a similar song automatically when the queue runs dry.</div>
      </div>
    </div>
    <div class="row" style="justify-content: flex-end; margin-top: 6px"><button class="btn primary" type="button" onclick={applyLive}>Apply limits</button></div>
  </div>
{/if}
