<script lang="ts">
  import { api } from '../../lib/api';
  import { form } from '../../lib/form.svelte';
  import { COOKIES_KEY, isField, type Field, type Special } from '../../lib/settings';
  import SecretField from './SecretField.svelte';

  let { entry }: { entry: Field | Special } = $props();

  function set(key: string, value: string) {
    form.values[key] = value;
    delete form.problems[key];
  }
  async function importCookies() {
    const relative = await api.importCookies();
    if (relative) form.values[COOKIES_KEY] = relative;
  }
</script>

{#if !isField(entry)}
  <div class="field">
    <span class="name">YouTube cookies</span>
    <div class="ctl">
      <div class="row">
        <button class="btn ghost sm" type="button" onclick={importCookies}>Import cookies.txt…</button>
        <button class="btn ghost sm" type="button" onclick={() => (form.values[COOKIES_KEY] = '')}>Clear</button>
        <span class="muted">{form.values[COOKIES_KEY] || 'Not set'}</span>
      </div>
      <div class="hint">Only needed for age-restricted or members-only videos. Export it from a browser extension in Netscape format.</div>
    </div>
  </div>
{:else}
  {@const id = `f-${entry.key}`}
  {@const problem = form.problems[entry.key]}
  <div class="field" class:bad={!!problem}>
    <label class="name" for={id}>{entry.label}</label>
    <div class="ctl">
      {#if entry.type === 'secret'}
        <SecretField {id} key={entry.key} />
      {:else if entry.type === 'bool'}
        <label class="switch">
          <input {id} type="checkbox" checked={form.values[entry.key] === 'true'} onchange={(e) => set(entry.key, String(e.currentTarget.checked))} />
          <span></span>
        </label>
      {:else if entry.type === 'select'}
        <select {id} class="input" value={form.values[entry.key]} onchange={(e) => set(entry.key, e.currentTarget.value)}>
          {#each entry.options ?? [] as option (option)}<option value={option}>{option}</option>{/each}
        </select>
      {:else}
        <input
          {id}
          class="input"
          class:num={entry.type === 'number'}
          type={entry.type === 'number' ? 'number' : 'text'}
          min={entry.min}
          max={entry.max}
          placeholder={entry.placeholder}
          autocomplete="off"
          spellcheck="false"
          value={form.values[entry.key] ?? ''}
          oninput={(e) => set(entry.key, e.currentTarget.value)}
        />
      {/if}
      {#if entry.hint}<div class="hint">{entry.hint}</div>{/if}
      {#if problem}<div class="bad-msg" role="alert">{problem}</div>{/if}
    </div>
  </div>
{/if}
