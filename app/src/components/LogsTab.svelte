<script lang="ts">
  import { tick } from 'svelte';
  import { api } from '../lib/api';
  import { splitMatches, stamp } from '../lib/format';
  import { LEVELS, clearLogs, filterLogs, levelKey, logView, logs } from '../lib/logs.svelte';
  import { copyText, toast } from '../lib/store.svelte';

  const ROW = 22; // px, must match .ll in app.css
  const OVERSCAN = 20;

  let viewport: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewHeight = $state(500);
  let queryText = $state(logView.query);
  let queryTimer: ReturnType<typeof setTimeout> | undefined;

  const view = $derived(filterLogs(logs.all, logView.levels, logView.query));
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN));
  const last = $derived(Math.min(view.length, Math.ceil((scrollTop + viewHeight) / ROW) + OVERSCAN));
  const rows = $derived(view.slice(first, last));

  function measure() {
    if (!viewport) return;
    scrollTop = viewport.scrollTop;
    viewHeight = viewport.clientHeight;
  }
  function onScroll() {
    if (!viewport) return;
    measure();
    logView.follow = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight < 30;
  }
  function toBottom() {
    void tick().then(() => {
      if (!viewport) return;
      viewport.scrollTop = viewport.scrollHeight;
      measure();
    });
  }
  $effect(() => {
    void view.length;
    if (logView.follow) toBottom();
  });
  $effect(() => {
    measure();
  });

  function toggleLevel(level: string) {
    const next = new Set(logView.levels);
    if (!next.delete(level)) next.add(level);
    logView.levels = next;
  }
  function onSearch() {
    clearTimeout(queryTimer);
    queryTimer = setTimeout(() => {
      logView.query = queryText;
      logView.follow = !queryText.trim();
      if (logView.follow) toBottom();
    }, 120);
  }
  function follow() {
    logView.follow = true;
    toBottom();
  }
  const line = (e: (typeof view)[number]) => `${stamp(e.ts)} [${levelKey(e.level)}] ${e.logger}: ${e.msg}`;
  function copyView() {
    if (!view.length) return toast('Nothing to copy.', 'err');
    void copyText(view.map(line).join('\n'), `${view.length} lines copied`);
  }
  async function exportLogs() {
    const result = await api.exportLogs();
    if (result.ok) toast('Logs exported.', 'ok');
    else if (!result.canceled) toast(result.error || "Couldn't export the logs.", 'err');
  }
  async function clear() {
    await api.clearLogs();
    clearLogs();
  }
  const LABELS: Record<string, string> = { DEBUG: 'Debug', INFO: 'Info', WARNING: 'Warning', ERROR: 'Error' };
</script>

<section class="tab tab-logs" aria-label="Logs">
  <div class="log-tools">
    <div class="chips" role="group" aria-label="Levels to show">
      {#each LEVELS as level (level)}
        <button class="chip" class:on={logView.levels.has(level)} data-level={level} aria-pressed={logView.levels.has(level)} onclick={() => toggleLevel(level)}>
          {LABELS[level]}
        </button>
      {/each}
    </div>
    <input class="input grow" type="search" placeholder="Search logs…" autocomplete="off" spellcheck="false" aria-label="Search logs" bind:value={queryText} oninput={onSearch} />
    <button class="btn ghost sm" class:on={logView.follow} aria-pressed={logView.follow} onclick={follow}>Follow</button>
    <button class="btn ghost sm" onclick={copyView}>Copy</button>
    <button class="btn ghost sm" onclick={exportLogs}>Export</button>
    <button class="btn ghost sm" onclick={() => api.openPath('logs')}>Log folder</button>
    <button class="btn ghost sm" onclick={clear}>Clear</button>
  </div>
  <div class="log-wrap">
    <!-- svelte-ignore a11y_no_noninteractive_tabindex: a scrollable region has to take focus so the keyboard can scroll it -->
    <div class="log-viewport" tabindex="0" role="log" aria-label="Log lines" bind:this={viewport} onscroll={onScroll}>
      <div class="log-sizer" style:height="{view.length * ROW}px">
        <div class="log-rows" style:top="{first * ROW}px">
          {#each rows as e (e.id)}
            <div class="ll {levelKey(e.level)}" class:cont={e.cont}>
              <span class="t">{stamp(e.ts)}</span><span class="lv">{levelKey(e.level)}</span><span class="lg">{e.logger}</span>
              <span class="m">{#each splitMatches(e.msg, logView.query.trim()) as part, i (i)}{#if part.match}<mark>{part.text}</mark>{:else}{part.text}{/if}{/each}</span>
            </div>
          {/each}
        </div>
      </div>
    </div>
    {#if !logView.follow && view.length}<button class="jump" onclick={follow}>Jump to latest ↓</button>{/if}
    {#if !view.length}<div class="empty overlay">No log lines match.</div>{/if}
  </div>
  <div class="log-foot muted small">
    <span>{view.length.toLocaleString()} of {logs.all.length.toLocaleString()} lines</span>
    <span>The log file on disk keeps the full history (rotated at 5 MB).</span>
  </div>
</section>
