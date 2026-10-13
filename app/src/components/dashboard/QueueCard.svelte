<script lang="ts">
  import { api } from '../../lib/api';
  import { bot, command, isLive, toast } from '../../lib/store.svelte';
  import Thumb from '../Thumb.svelte';

  const live = $derived(isLive());
  const s = $derived(live ? bot.snap : null);
  const items = $derived(s?.queue ?? []);
  const more = $derived(s && s.queue_size > items.length ? s.queue_size - items.length : 0);

  // clearing needs a second click
  let armed = $state(false);
  let armTimer: ReturnType<typeof setTimeout> | undefined;
  function clear() {
    if (!armed) {
      armed = true;
      armTimer = setTimeout(() => (armed = false), 3000);
      return;
    }
    clearTimeout(armTimer);
    armed = false;
    void command('clear_queue', 'Queue cleared.');
  }

  // Radio lookahead: keeps the next N songs of the radio mix queued behind whatever was requested.
  // It lives here and nowhere else: not in chat commands, not in the browser settings page.
  let lookahead = $state({ enabled: true, count: 5 });
  let countText = $state('5');
  let countInput: HTMLInputElement | undefined = $state();
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let holdUntil = 0; // after the person changes it, ignore reports that may predate the change

  $effect(() => {
    // While the bot runs, what it reports is the truth; the person's own edit wins while they are
    // typing and for a moment after, until the bot has had time to pick the change up.
    const reported = s?.radio_lookahead;
    if (live && reported && !saveTimer && Date.now() >= holdUntil) lookahead = { ...reported };
  });
  $effect(() => {
    const value = String(lookahead.count);
    if (document.activeElement !== countInput) countText = value;
  });
  $effect(() => {
    void api.getLookahead().then((saved) => {
      if (!live) lookahead = saved;
    }).catch(() => {});
  });

  async function save(patch: Partial<typeof lookahead>) {
    lookahead = { ...lookahead, ...patch };
    holdUntil = Date.now() + 3000;
    try {
      lookahead = await api.setLookahead(lookahead);
    } catch {
      toast("Couldn't save the radio lookahead setting.", 'err');
    }
  }
  function onCount() {
    clearTimeout(saveTimer);
    const n = Number.parseInt(countText, 10);
    if (!Number.isFinite(n)) return;
    saveTimer = setTimeout(() => {
      saveTimer = undefined;
      void save({ count: Math.min(15, Math.max(1, n)) });
    }, 500);
  }

  const radioOff = $derived(live && s?.radio_autoplay === false);
  const hint = $derived(
    radioOff
      ? 'Auto-radio is off (Settings, or !radio on), so no radio songs are queued.'
      : lookahead.enabled
        ? `Keeps the next ${lookahead.count} song${lookahead.count === 1 ? '' : 's'} of the radio mix queued, like YouTube Music.`
        : 'Off: a radio song is added only when the queue runs dry.',
  );
</script>

<div class="card queue">
  <div class="card-head">
    <h2>Queue</h2>
    <button class="btn ghost sm" class:armed disabled={!(live && s && s.queue_size > 0)} onclick={clear}>{armed ? 'Click again to clear' : 'Clear'}</button>
  </div>
  {#if items.length}
    <ol class="qlist">
      {#each items as item, i (i)}
        <li class:radio={item.radio}>
          <Thumb src={item.thumbnail} cls="q-thumb" />
          <span class="q-title">{item.title}</span>
          <span class="q-by">{item.radio ? 'radio mix' : item.requester}</span>
        </li>
      {/each}
      {#if more}<li><span class="q-title muted">…and {more} more</span></li>{/if}
    </ol>
  {:else}
    <div class="empty">The queue is empty.</div>
  {/if}
  <div class="lookahead">
    <span class="name">Radio lookahead</span>
    <label class="switch">
      <input type="checkbox" aria-label="Radio lookahead" checked={lookahead.enabled} onchange={(e) => save({ enabled: e.currentTarget.checked })} />
      <span></span>
    </label>
    <input
      bind:this={countInput}
      bind:value={countText}
      class="input num"
      type="number"
      min="1"
      max="15"
      step="1"
      aria-label="Songs to line up from the radio mix"
      disabled={!lookahead.enabled}
      oninput={onCount}
      onblur={() => (countText = String(lookahead.count))}
    />
    <span class="muted small">{hint}</span>
  </div>
</div>
