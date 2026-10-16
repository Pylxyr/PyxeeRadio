<script lang="ts">
  import { fmtDur } from '../../lib/format';
  import { bot, clock, command, isLive } from '../../lib/store.svelte';
  import Icon from '../Icon.svelte';

  const live = $derived(isLive());
  const s = $derived(live ? bot.snap : null);
  const np = $derived(s?.now ?? null);
  let failedThumb = $state('');

  const elapsed = $derived.by(() => {
    if (!np || !s) return 0;
    const running = s.player.paused ? 0 : Math.max(0, clock.now - bot.snapAt) / 1000;
    const value = np.elapsed + running;
    return np.duration > 0 ? Math.min(value, np.duration) : value;
  });
  const percent = $derived(np && np.duration > 0 ? Math.min(100, (elapsed / np.duration) * 100) : 0);
  const paused = $derived(!!s?.player.paused);
  // Pausing remembers where the song was and Resume carries on from there; say so.
  const resumeTitle = $derived(s?.queue[0]?.title ?? '');
  const resumeAt = $derived(s?.queue[0]?.resume_at ?? 0);
  const sub = $derived(
    np
      ? np.uploader || ''
      : paused
        ? resumeTitle
          ? resumeAt > 0
            ? `Resume continues “${resumeTitle}” from ${fmtDur(resumeAt)}.`
            : `Resume plays “${resumeTitle}”.`
          : 'Press Resume to continue.'
        : live
          ? 'Add a song above, or type !sr <song or link> in your chat.'
          : 'Start the bot, then add a song above or use !sr in chat.',
  );
  const skipHint = $derived(np && s?.player.next_ready === false ? 'The next song is still loading; skip will wait for it.' : '');
</script>

<div class="card now">
  <div class="card-head">
    <h2>Now playing</h2>
    <div class="row">
      <button class="btn sm" class:primary={paused} class:ghost={!paused} disabled={!live} onclick={() => command(paused ? 'resume' : 'pause')}>{paused ? 'Resume' : 'Pause'}</button>
      <button class="btn ghost sm" disabled={!np} title={skipHint} onclick={() => command('skip')}>Skip</button>
    </div>
  </div>
  <div class="now-body">
    <div class="art">
      {#if np?.thumbnail && failedThumb !== np.thumbnail}
        <img src={np.thumbnail} alt="" onerror={() => (failedThumb = np.thumbnail ?? '')} />
      {:else}
        <div class="art-ph"><Icon name="music" /></div>
      {/if}
    </div>
    <div class="now-meta">
      <div class="now-title">{np ? np.title : paused ? 'Paused' : live ? 'Waiting for a request…' : 'Nothing playing'}</div>
      <div class="muted">{sub}</div>
      <div class="progress"><div class="bar" style:width="{percent.toFixed(1)}%"></div></div>
      <div class="row between small muted"><span>{fmtDur(elapsed)}</span><span>{np ? (np.duration > 0 ? fmtDur(np.duration) : 'live') : '0:00'}</span></div>
      <div class="tags">
        {#if np}
          <span class="tag" class:purple={!np.radio}>{np.radio ? 'Auto radio' : `Requested by ${np.requester}`}</span>
        {/if}
        {#if paused}<span class="tag warn">Paused</span>{/if}
      </div>
    </div>
  </div>
</div>
