<script lang="ts">
  import { api } from '../../lib/api';
  import { fmtDur, looksLikeLink } from '../../lib/format';
  import { isLive } from '../../lib/store.svelte';
  import type { SearchResult } from '../../lib/types';
  import Thumb from '../Thumb.svelte';

  let text = $state('');
  let requesting = $state(false);
  let searching = $state(false);
  let status = $state<{ message: string; kind: '' | 'ok' | 'err' }>({ message: '', kind: '' });
  let results = $state<SearchResult[]>([]);
  let input: HTMLInputElement | undefined = $state();

  const live = $derived(isLive());
  const busy = $derived(requesting || searching);
  const buttonLabel = $derived(requesting ? 'Adding…' : searching ? 'Searching…' : looksLikeLink(text.trim()) ? 'Add to queue' : 'Search');

  async function addSong(query: string) {
    if (requesting || !live) return;
    requesting = true;
    results = [];
    status = { message: 'Adding…', kind: '' };
    try {
      const result = await api.requestSong(query);
      if (result?.ok) {
        text = '';
        status = { message: `Added: ${result.title} (#${result.position} in the queue)`, kind: 'ok' };
      } else status = { message: result?.error || 'Could not add that song.', kind: 'err' };
    } catch {
      status = { message: 'Could not reach the bot.', kind: 'err' };
    } finally {
      requesting = false;
      input?.focus();
    }
  }

  async function searchSongs(query: string) {
    if (busy || !live) return;
    searching = true;
    results = [];
    status = { message: 'Searching…', kind: '' };
    try {
      const result = await api.searchSongs(query);
      if (result?.ok && result.results.length) {
        status = { message: '', kind: '' };
        results = result.results;
      } else status = { message: result?.error || 'No results for that.', kind: 'err' };
    } catch {
      status = { message: 'Could not reach the bot.', kind: 'err' };
    } finally {
      searching = false;
    }
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    const query = text.trim();
    if (!query) return;
    // A link is already a choice; anything else is searched, and the person picks the match.
    if (looksLikeLink(query)) void addSong(query);
    else void searchSongs(query);
  }
</script>

<div class="card search">
  <form autocomplete="off" onsubmit={submit}>
    <input
      bind:this={input}
      bind:value={text}
      class="input"
      type="search"
      maxlength="300"
      placeholder="Search YouTube or paste a link to add a song to the queue…"
      spellcheck="false"
      disabled={!live || requesting}
      title={live ? '' : 'Start the bot first'}
      aria-label="Song to add"
      oninput={() => (results = [])}
      onkeydown={(e) => e.key === 'Escape' && (results = [])}
    />
    <button class="btn primary" type="submit" disabled={!live || busy}>{buttonLabel}</button>
  </form>
  <div class="search-status {status.kind}" aria-live="polite">{status.message}</div>
  {#if results.length}
    <div class="results">
      <div class="results-head"><span>Pick the song to add</span><button class="btn ghost sm" type="button" onclick={() => (results = [])}>Close</button></div>
      <ul>
        {#each results as item (item.url)}
          <li>
            <button class="result" type="button" title="Add this song to the queue" onclick={() => addSong(item.url)}>
              <Thumb src={item.thumbnail} cls="r-thumb" />
              <span class="r-text">
                <span class="r-title">{item.title}</span>
                <span class="r-by">{[item.uploader, item.duration ? fmtDur(item.duration) : ''].filter(Boolean).join(' · ')}</span>
              </span>
              <span class="r-add">Add</span>
            </button>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</div>
