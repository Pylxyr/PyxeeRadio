<script lang="ts">
  import { api } from '../../lib/api';
  import { bot, info, prefs, refreshPreflight, setPrefs, toast, upd } from '../../lib/store.svelte';

  const busy = $derived(!!upd.busy);
  const a = $derived(upd.app);
  const appStatus = $derived(
    !a ? `v${info.version}` : !a.ok ? `v${info.version} - ${a.error}` : a.updateAvailable ? `v${info.version} - v${a.latest} is available` : `v${info.version} - up to date`,
  );
  const y = $derived.by(() => {
    const report = upd.ytdlp;
    const base = bot.pre?.ytdlp;
    return {
      version: report?.current ?? base?.version,
      source: report?.source ?? base?.source,
      previous: report?.previous !== undefined ? report.previous : base?.previous,
      latest: report?.latest ?? null,
      available: !!report?.update_available,
      error: report?.error ?? null,
      overrideError: report?.override_error ?? base?.override_error ?? null,
    };
  });
  const ytStatus = $derived(
    y.version ? `${y.version}${y.source === 'override' ? ' (updated)' : ' (bundled)'} - latest: ${y.latest || 'not checked'}${y.error ? ` (${y.error})` : ''}` : 'unknown',
  );
  const solver = $derived(bot.pre?.js_solver);

  const message = (error: unknown) => (error instanceof Error ? error.message : String(error));

  async function checkApp() {
    upd.busy = 'app';
    try {
      upd.app = await api.checkAppUpdate();
    } catch (error) {
      upd.app = { ok: false, current: info.version, latest: null, updateAvailable: false, releaseUrl: null, installerUrl: null, error: message(error) };
    }
    upd.busy = '';
  }
  async function checkYtdlp() {
    upd.busy = 'check';
    try {
      upd.ytdlp = await api.ytdlpCheck();
    } catch (error) {
      upd.ytdlp = { ok: false, error: message(error) };
    }
    upd.busy = '';
  }
  async function changeYtdlp(kind: 'install' | 'rollback') {
    upd.busy = kind;
    try {
      const result = kind === 'install' ? await api.ytdlpInstall() : await api.ytdlpRollback();
      if (result.ok) toast(kind === 'install' ? 'yt-dlp updated.' : 'Rolled back yt-dlp.', 'ok');
      else toast(result.error || "Couldn't change yt-dlp.", 'err');
    } catch (error) {
      toast(`Couldn't change yt-dlp: ${message(error)}`, 'err');
    }
    upd.busy = '';
    await refreshPreflight();
    upd.ytdlp = await api.ytdlpCheck().catch(() => upd.ytdlp);
  }
</script>

<div class="card">
  <div class="card-head"><h2>Updates</h2></div>
  <div class="field">
    <span class="name">This app</span>
    <div class="ctl">
      <div class="row">
        <span class="muted">{appStatus}</span>
        <button class="btn ghost sm" type="button" disabled={busy} onclick={checkApp}>{upd.busy === 'app' ? 'Checking…' : 'Check for app updates'}</button>
        {#if a?.ok && a.updateAvailable}
          <button class="btn primary sm" type="button" onclick={() => api.openExternal((a.installerUrl || a.releaseUrl) ?? '')}>Download installer</button>
        {/if}
      </div>
      <div class="hint">Only checks and links to the installer; nothing is installed for you.</div>
    </div>
  </div>
  <div class="field">
    <label class="name" for="p-checkAppUpdates">Check for app updates automatically (once a day)</label>
    <div class="ctl"><label class="switch"><input id="p-checkAppUpdates" type="checkbox" checked={prefs.checkAppUpdates} onchange={(e) => setPrefs({ checkAppUpdates: e.currentTarget.checked })} /><span></span></label></div>
  </div>
  <div class="field">
    <span class="name">yt-dlp</span>
    <div class="ctl">
      <div class="row">
        <span class="muted">{ytStatus}</span>
        <button class="btn ghost sm" type="button" disabled={busy} onclick={checkYtdlp}>{upd.busy === 'check' ? 'Checking…' : 'Check for updates'}</button>
        <button class="btn primary sm" type="button" disabled={busy || !y.available} onclick={() => changeYtdlp('install')}>{upd.busy === 'install' ? 'Updating…' : 'Update'}</button>
        {#if y.previous}<button class="btn ghost sm" type="button" disabled={busy} onclick={() => changeYtdlp('rollback')}>Roll back to {y.previous}</button>{/if}
      </div>
      <div class="hint">YouTube changes often; updating yt-dlp does not need a new installer. Updating restarts the bot if it is running.</div>
    </div>
  </div>
  {#if y.overrideError}
    <div class="hint err-text">The downloaded yt-dlp failed to load, so the bundled one is in use ({y.overrideError}).</div>
  {/if}
  <div class="field">
    <label class="name" for="p-checkYtdlpOnStartup">Check for yt-dlp updates on startup</label>
    <div class="ctl"><label class="switch"><input id="p-checkYtdlpOnStartup" type="checkbox" checked={prefs.checkYtdlpOnStartup} onchange={(e) => setPrefs({ checkYtdlpOnStartup: e.currentTarget.checked })} /><span></span></label></div>
  </div>
  <div class="field">
    <span class="name">JS solver</span>
    <div class="ctl">
      <div class="row"><span class="tag" class:purple={!!solver?.ready}>{!solver ? 'unknown' : solver.ready ? 'ready' : 'not downloaded yet (optional)'}</span></div>
      <div class="hint">
        {solver?.ready
          ? 'Downloaded by yt-dlp from github.com/yt-dlp/ejs and kept in the data folder.'
          : 'Only needed if YouTube starts requiring it, for example on a datacenter IP. yt-dlp then downloads it from github.com on its own; if that is blocked, allow github.com through your firewall or antivirus.'}
      </div>
    </div>
  </div>
</div>
