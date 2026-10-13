<script lang="ts">
  import { queueSummary } from '../../lib/health';
  import { fmtUptime } from '../../lib/format';
  import { bot, clock, isLive } from '../../lib/store.svelte';

  const live = $derived(isLive());
  const s = $derived(live ? bot.snap : null);
  const uptime = $derived(
    live && bot.status.startedAt && bot.snap ? fmtUptime(bot.snap.uptime + Math.max(0, clock.now - bot.snapAt) / 1000) : '–',
  );
  const since = $derived(
    live && bot.status.startedAt
      ? `Since ${new Date(bot.status.startedAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`
      : 'Not running',
  );
</script>

<div class="kpis">
  <div class="card kpi"><div class="kpi-label">Uptime</div><div class="kpi-value">{uptime}</div><div class="kpi-sub">{since}</div></div>
  <div class="card kpi"><div class="kpi-label">Listeners</div><div class="kpi-value">{s ? s.player.listeners : '–'}</div><div class="kpi-sub">OBS audio connections</div></div>
  <div class="card kpi"><div class="kpi-label">In queue</div><div class="kpi-value">{s ? s.queue_size : '–'}</div><div class="kpi-sub">{s ? queueSummary(s) : '\u00a0'}</div></div>
  <div class="card kpi">
    <div class="kpi-label">Songs played <span class="muted">· last hour</span></div>
    <div class="kpi-value">{s ? s.counters.tracks_played : '–'}</div>
    <div class="kpi-sub">{s ? `${s.counters.skips} skipped · ${s.counters.tracks_failed} failed` : '\u00a0'}</div>
  </div>
</div>
