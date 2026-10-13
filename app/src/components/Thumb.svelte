<script lang="ts">
  // A song's picture, or the plain placeholder when there is none or it fails to load.
  let { src, cls }: { src?: string | null; cls: string } = $props();
  let failedSrc = $state('');
</script>

{#if src && failedSrc !== src}
  <img class={cls} {src} alt="" loading="lazy" onerror={() => (failedSrc = src ?? '')} />
{:else}
  <span class="{cls} ph"></span>
{/if}
