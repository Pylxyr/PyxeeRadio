<script lang="ts">
  import { api } from '../../lib/api';
  import { OAUTH_REDIRECT, setupSteps } from '../../lib/health';
  import { bot, copyText, goto, isLive } from '../../lib/store.svelte';

  const steps = $derived(setupSteps({ live: isLive(), snap: bot.snap, pre: bot.pre }));
  const done = $derived(steps.filter((s) => s.done).length);
  const show = $derived(bot.pre !== null && done < steps.length);

  const live = $derived(isLive());
</script>

{#if show}
  <div class="card setup">
    <div class="card-head"><h2>Finish setting up</h2><span class="muted small">{done} of {steps.length} done</span></div>
    <ol class="steps">
      {#each steps as step (step.id)}
        <li class:done={step.done}>
          <span class="check">{step.done ? '✓' : ''}</span>
          <div class="step-body">
            <div class="step-title">{step.title}</div>
            {#if step.desc && !step.done}<div class="step-desc">{step.desc}</div>{/if}
          </div>
          {#if !step.done}
            <div class="row">
              {#if step.id === 'credentials'}
                <button class="btn primary sm" onclick={() => goto('settings')}>Open Settings</button>
                <button class="btn ghost sm" onclick={() => copyText(OAUTH_REDIRECT, 'Redirect URL copied')}>Copy redirect URL</button>
              {:else if step.url}
                <button
                  class="btn primary sm"
                  disabled={!!step.needsBot && !live}
                  title={step.needsBot && !live ? 'Start the bot first' : ''}
                  onclick={() => api.openExternal(step.url!)}
                >Authorize</button>
                <button class="btn ghost sm" onclick={() => copyText(step.url!, 'Link copied')}>Copy link</button>
              {/if}
            </div>
          {/if}
        </li>
      {/each}
    </ol>
  </div>
{/if}
